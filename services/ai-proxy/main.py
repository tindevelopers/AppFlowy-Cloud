"""
Lightweight replacement for the closed-source, license-gated `appflowy_ai`
service. Implements the subset of the AppFlowy AI HTTP contract
(https://github.com/AppFlowy-IO/AppFlowy-Cloud/blob/main/libs/appflowy-ai-client)
that the desktop/web client actually exercises (chat, AI writer completion,
model list, health), backed directly by the OpenAI Chat Completions API.

Response envelope matches `AIResponse<T>` on the Rust side:
    {"data": <T> | null, "message": "<str>"}

Streaming endpoints return a raw sequence of concatenated JSON objects
(no delimiter required, no SSE "data:" framing) with a single key per
chunk, matching `STREAM_METADATA_KEY` ("0") / `STREAM_ANSWER_KEY` ("1") /
`STREAM_IMAGE_KEY` ("2") / `STREAM_KEEP_ALIVE_KEY` ("3") /
`STREAM_COMMENT_KEY` ("4") from `appflowy-ai-client/src/dto.rs`.
"""
import json
import logging
import os
from typing import Any, AsyncIterator

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse, StreamingResponse
from openai import AsyncOpenAI

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("ai-proxy")

OPENAI_API_KEY = os.environ.get("OPENAI_API_KEY", "")
AZURE_OPENAI_API_KEY = os.environ.get("AZURE_OPENAI_API_KEY", "")
AZURE_OPENAI_ENDPOINT = os.environ.get("AZURE_OPENAI_ENDPOINT", "")
AZURE_OPENAI_API_VERSION = os.environ.get("AZURE_OPENAI_API_VERSION", "")
DEFAULT_AI_MODEL = os.environ.get("DEFAULT_AI_MODEL", "gpt-4o-mini")
DEFAULT_AI_COMPLETION_MODEL = os.environ.get(
    "DEFAULT_AI_COMPLETION_MODEL", DEFAULT_AI_MODEL
)

if AZURE_OPENAI_API_KEY and AZURE_OPENAI_ENDPOINT:
    from openai import AsyncAzureOpenAI

    client = AsyncAzureOpenAI(
        api_key=AZURE_OPENAI_API_KEY,
        api_version=AZURE_OPENAI_API_VERSION or "2024-02-01",
        azure_endpoint=AZURE_OPENAI_ENDPOINT,
    )
else:
    client = AsyncOpenAI(api_key=OPENAI_API_KEY)

app = FastAPI(title="appflowy-ai-proxy")

STREAM_ANSWER_KEY = "1"
STREAM_COMMENT_KEY = "4"


def envelope(data: Any = None, message: str = "") -> dict:
    return {"data": data, "message": message}


def resolve_model(header_value: str | None, default: str) -> str:
    if not header_value or header_value.strip().lower() == "default":
        return default
    return header_value


async def read_json(request: Request) -> dict:
    try:
        return await request.json()
    except Exception:
        return {}


def chunk_bytes(key: str, text: str) -> bytes:
    return json.dumps({key: text}, ensure_ascii=False).encode("utf-8")


@app.get("/health")
async def health():
    return JSONResponse({"status": "healthy", "self_hosted": True})


@app.get("/model/list")
async def model_list():
    models = [{"name": DEFAULT_AI_MODEL, "metadata": None}]
    if DEFAULT_AI_COMPLETION_MODEL != DEFAULT_AI_MODEL:
        models.append({"name": DEFAULT_AI_COMPLETION_MODEL, "metadata": None})
    return JSONResponse(envelope({"models": models}))


async def chat_completion_text(model: str, content: str) -> str:
    resp = await client.chat.completions.create(
        model=model,
        messages=[{"role": "user", "content": content}],
    )
    return resp.choices[0].message.content or ""


async def stream_chat_completion(model: str, content: str) -> AsyncIterator[bytes]:
    try:
        stream = await client.chat.completions.create(
            model=model,
            messages=[{"role": "user", "content": content}],
            stream=True,
        )
        async for chunk in stream:
            delta = chunk.choices[0].delta.content if chunk.choices else None
            if delta:
                yield chunk_bytes(STREAM_ANSWER_KEY, delta)
    except Exception as err:  # noqa: BLE001
        logger.exception("chat completion stream failed")
        yield chunk_bytes(STREAM_COMMENT_KEY, f"AI error: {err}")


@app.post("/chat/message")
async def chat_message(request: Request):
    body = await read_json(request)
    content = ((body.get("data") or {}).get("content")) or ""
    model = resolve_model(request.headers.get("ai-model"), DEFAULT_AI_MODEL)
    try:
        answer = await chat_completion_text(model, content)
    except Exception as err:  # noqa: BLE001
        logger.exception("chat_message failed")
        return JSONResponse(envelope(message=str(err)), status_code=502)
    return JSONResponse(envelope({"content": answer, "metadata": None}))


async def _chat_stream_endpoint(request: Request):
    body = await read_json(request)
    content = ((body.get("data") or {}).get("content")) or ""
    model = resolve_model(request.headers.get("ai-model"), DEFAULT_AI_MODEL)
    return StreamingResponse(
        stream_chat_completion(model, content), media_type="text/event-stream"
    )


@app.post("/chat/message/stream")
async def chat_message_stream(request: Request):
    return await _chat_stream_endpoint(request)


@app.post("/v2/chat/message/stream")
async def chat_message_stream_v2(request: Request):
    return await _chat_stream_endpoint(request)


COMPLETION_INSTRUCTIONS = {
    1: "Improve the writing of the following text. Preserve its original meaning, tone, and language. Return only the improved text.",
    2: "Fix the spelling and grammar mistakes in the following text without changing its meaning. Return only the corrected text.",
    3: "Rewrite the following text to be more concise while preserving its meaning. Return only the shortened text.",
    4: "Expand the following text with more relevant detail while preserving its meaning and tone. Return only the expanded text.",
    5: "Continue writing from where the following text leaves off, matching its style and tone. Return only the continuation.",
    6: "Explain the following text clearly and concisely.",
    7: None,  # AskAI: treat text as a direct instruction/question
    8: None,  # CustomPrompt: system prompt supplied via metadata.custom_prompt.system
}


def build_completion_messages(body: dict) -> list[dict]:
    text = body.get("text") or ""
    completion_type = body.get("completion_type")
    metadata = body.get("metadata") or {}
    custom_prompt = (metadata.get("custom_prompt") or {}).get("system")

    if completion_type == 8 and custom_prompt:
        return [
            {"role": "system", "content": custom_prompt},
            {"role": "user", "content": text},
        ]

    instruction = COMPLETION_INSTRUCTIONS.get(completion_type)
    if instruction:
        return [
            {"role": "system", "content": instruction},
            {"role": "user", "content": text},
        ]

    # AskAI or unspecified: treat text as a direct instruction/question.
    return [{"role": "user", "content": text}]


async def stream_completion(model: str, messages: list[dict]) -> AsyncIterator[bytes]:
    try:
        stream = await client.chat.completions.create(
            model=model, messages=messages, stream=True
        )
        async for chunk in stream:
            delta = chunk.choices[0].delta.content if chunk.choices else None
            if delta:
                yield chunk_bytes(STREAM_ANSWER_KEY, delta)
    except Exception as err:  # noqa: BLE001
        logger.exception("completion stream failed")
        yield chunk_bytes(STREAM_COMMENT_KEY, f"AI error: {err}")


async def _completion_stream_endpoint(request: Request):
    body = await read_json(request)
    messages = build_completion_messages(body)
    model = resolve_model(request.headers.get("ai-model"), DEFAULT_AI_COMPLETION_MODEL)
    return StreamingResponse(
        stream_completion(model, messages), media_type="text/event-stream"
    )


@app.post("/completion/stream")
async def completion_stream(request: Request):
    return await _completion_stream_endpoint(request)


@app.post("/v2/completion/stream")
async def completion_stream_v2(request: Request):
    return await _completion_stream_endpoint(request)


@app.post("/summarize_row")
async def summarize_row(request: Request):
    fields = await read_json(request)
    model = resolve_model(request.headers.get("ai-model"), DEFAULT_AI_MODEL)
    if not fields:
        return JSONResponse(envelope({"text": "No content"}))

    rows_text = "\n".join(f"{k}: {v}" for k, v in fields.items())
    prompt = (
        "Summarize the following database row in one short sentence.\n\n" + rows_text
    )
    try:
        text = await chat_completion_text(model, prompt)
    except Exception:
        logger.exception("summarize_row failed")
        text = "No content"
    return JSONResponse(envelope({"text": text.strip() or "No content"}))


@app.post("/translate_row")
async def translate_row(request: Request):
    body = await read_json(request)
    cells = body.get("cells") or []
    language = body.get("language") or "English"
    include_header = bool(body.get("include_header"))
    model = resolve_model(request.headers.get("ai-model"), DEFAULT_AI_MODEL)

    items: list[dict] = []
    for cell in cells:
        title = cell.get("title") or ""
        content = cell.get("content") or ""
        try:
            translated_content = await chat_completion_text(
                model,
                f"Translate the following text to {language}. Return only the translation:\n\n{content}",
            )
        except Exception:
            logger.exception("translate_row failed for a cell")
            translated_content = content

        item = {"content": translated_content.strip()}
        if include_header and title:
            try:
                item["title"] = (
                    await chat_completion_text(
                        model,
                        f"Translate the following text to {language}. Return only the translation:\n\n{title}",
                    )
                ).strip()
            except Exception:
                item["title"] = title
        items.append(item)

    return JSONResponse(envelope({"items": items}))


@app.post("/chat/context/text")
async def chat_context_text(_: Request):
    # Not implemented: chat-with-document context (RAG) is handled by the
    # separate appflowy_search service in this deployment. Accept and no-op
    # so callers relying on this succeeding don't error out.
    return JSONResponse(envelope())


@app.post("/chat/image/regenerate")
async def chat_image_regenerate(_: Request):
    # Image generation is not implemented in this proxy.
    return JSONResponse(envelope())


@app.get("/search")
async def search_documents():
    # Semantic document search is handled by the appflowy_search service;
    # returning an empty list here is a safe no-op for this endpoint.
    return JSONResponse(envelope([]))


@app.post("/similarity")
async def similarity(_: Request):
    return JSONResponse(envelope({"score": 0.0}))


@app.get("/local_ai/plugin")
async def local_ai_plugin():
    return JSONResponse(envelope([]))


@app.get("/local_ai/config")
async def local_ai_config():
    return JSONResponse(
        envelope(
            {
                "models": [],
                "plugin": {
                    "app_name": "AppFlowy",
                    "ai_plugin_name": "",
                    "version": "",
                    "url": "",
                    "etag": "",
                },
            }
        )
    )


@app.get("/chat/{chat_id}/{message_id}/related_question")
async def related_question(chat_id: str, message_id: int):
    return JSONResponse(envelope({"message_id": message_id, "items": []}))
