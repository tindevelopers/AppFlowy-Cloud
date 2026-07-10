FROM python:3.12-slim

RUN apt-get update && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY main.py .

ENV AI_SERVER_PORT=5001
EXPOSE 5001

CMD ["sh", "-c", "uvicorn main:app --host 0.0.0.0 --port ${AI_SERVER_PORT:-5001}"]
