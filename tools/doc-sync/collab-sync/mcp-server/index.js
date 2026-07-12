#!/usr/bin/env node
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import path from "node:path";
import { fileURLToPath } from "node:url";

const exec = promisify(execFile);
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BIN = path.join(__dirname, "..", "target", "debug", "collab-sync");

const DEFAULT_WS = "d7cc1938-4ed2-4b8d-a9b6-4cec3c3bc046";
const SOP_WS = "24537940-119a-49f3-b3b7-a34c84ebb7e5";

function build() {
  return exec("cargo", ["build"], { cwd: path.join(__dirname, "..") });
}

async function run(args, cwd = path.join(__dirname, "..")) {
  const { stdout, stderr } = await exec(BIN, args, { cwd, env: { ...process.env } });
  return stdout + (stderr ? "\n[stderr]\n" + stderr : "");
}

const server = new Server(
  { name: "appflowy-collab-sync", version: "1.0.0" },
  { capabilities: { tools: {} } }
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: "sync_all",
      description: "Sync all pages from a manifest to AppFlowy. Optionally regenerate docs first.",
      inputSchema: {
        type: "object",
        properties: {
          workspace: { type: "string", enum: ["source-code", "sops"], description: "Which workspace to sync" },
          dry_run: { type: "boolean", description: "Preview changes without writing" },
          backup: { type: "boolean", description: "Save pre-change backup" },
          regenerate: { type: "boolean", description: "Run regenerate-docs.py first" },
        },
        required: ["workspace"],
      },
    },
    {
      name: "sync_page",
      description: "Sync a single page by view ID and markdown file path.",
      inputSchema: {
        type: "object",
        properties: {
          view_id: { type: "string", description: "Page view ID (UUID)" },
          md_path: { type: "string", description: "Path to markdown file (relative to collab-sync dir)" },
          section: { type: "string", description: "Section key (default: main)" },
          workspace: { type: "string", description: "Workspace ID (UUID), defaults to Source Code workspace" },
          dry_run: { type: "boolean" },
          backup: { type: "boolean" },
        },
        required: ["view_id", "md_path"],
      },
    },
    {
      name: "list_workspace",
      description: "List all pages in a workspace with their view IDs.",
      inputSchema: {
        type: "object",
        properties: {
          workspace_id: { type: "string", description: "Workspace ID (UUID), default: Source Code" },
        },
      },
    },
    {
      name: "fetch_page",
      description: "Fetch and display a page's full block tree and marker scan.",
      inputSchema: {
        type: "object",
        properties: {
          view_id: { type: "string", description: "Page view ID (UUID)" },
          workspace_id: { type: "string", description: "Workspace ID (UUID), defaults to Source Code" },
        },
        required: ["view_id"],
      },
    },
    {
      name: "regenerate_docs",
      description: "Regenerate markdown documentation from the codebase.",
      inputSchema: {
        type: "object",
        properties: {},
      },
    },
    {
      name: "restore_page",
      description: "Restore a page from a backup file.",
      inputSchema: {
        type: "object",
        properties: {
          view_id: { type: "string", description: "Page view ID (UUID)" },
          backup_path: { type: "string", description: "Path to backup .doc_state file" },
          workspace_id: { type: "string", description: "Workspace ID (UUID)" },
        },
        required: ["view_id", "backup_path"],
      },
    },
  ],
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  try {
    const { name, arguments: args } = request.params;
    await build();

    switch (name) {
      case "sync_all": {
        const ws = args.workspace === "sops" ? SOP_WS : DEFAULT_WS;
        const manifest = args.workspace === "sops" ? "manifest-sop.json" : "manifest.json";
        const flags = [
          (args.dry_run ? "--dry-run" : ""),
          (args.backup ? "--backup" : ""),
        ].filter(Boolean);
        if (args.regenerate) {
          await exec("python3", ["regenerate-docs.py"], { cwd: path.join(__dirname, "..") });
        }
        const output = await run(["sync", "--all", "--manifest", manifest, "--workspace", ws, ...flags]);
        return { content: [{ type: "text", text: output }] };
      }

      case "sync_page": {
        const ws = args.workspace || DEFAULT_WS;
        const section = args.section || "main";
        const flags = [
          "--section", section,
          "--workspace", ws,
          (args.dry_run ? "--dry-run" : ""),
          (args.backup ? "--backup" : ""),
        ].filter(Boolean);
        const output = await run(["sync", args.view_id, args.md_path, ...flags]);
        return { content: [{ type: "text", text: output }] };
      }

      case "list_workspace": {
        const ws = args.workspace_id || DEFAULT_WS;
        const output = await run(["list", ws]);
        return { content: [{ type: "text", text: output }] };
      }

      case "fetch_page": {
        const ws = args.workspace_id || DEFAULT_WS;
        const output = await run(["fetch", args.view_id, "--workspace", ws]);
        return { content: [{ type: "text", text: output }] };
      }

      case "regenerate_docs": {
        const { stdout, stderr } = await exec("python3", ["regenerate-docs.py"], { cwd: path.join(__dirname, "..") });
        return { content: [{ type: "text", text: stdout + (stderr || "") }] };
      }

      case "restore_page": {
        const ws = args.workspace_id || DEFAULT_WS;
        const output = await run(["restore", args.view_id, args.backup_path, "--workspace", ws]);
        return { content: [{ type: "text", text: output }] };
      }

      default:
        return { content: [{ type: "text", text: `Unknown tool: ${name}` }], isError: true };
    }
  } catch (e) {
    return { content: [{ type: "text", text: String(e) }], isError: true };
  }
});

const transport = new StdioServerTransport();
await server.connect(transport);
