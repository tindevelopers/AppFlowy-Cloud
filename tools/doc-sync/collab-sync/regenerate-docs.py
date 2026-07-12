#!/usr/bin/env python3
"""Doc regeneration script - scans the AppFlowy codebase and regenerates
markdown documentation for each tracked page.

TODO: Customize the scan/extraction logic for each page below.
Currently regenerates from static template sections.
"""

import os, sys

REPO_ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
CONTENT_DIR = os.path.join(os.path.dirname(__file__), "..", "importer", "content")

def regenerate_flutter_frontend():
    """Scan frontend/appflowy_flutter/lib/ for structure."""
    lib_dir = os.path.join(REPO_ROOT, "frontend", "appflowy_flutter", "lib")
    if not os.path.isdir(lib_dir):
        return "# Flutter Frontend\n\n(lib/ directory not found - run from repo root)\n"

    dirs = sorted(d for d in os.listdir(lib_dir) if os.path.isdir(os.path.join(lib_dir, d)))
    lines = ["# Flutter Frontend", "",
             "The desktop and mobile client for AppFlowy, written in Dart/Flutter.", "",
             "## Location", "", f"`frontend/appflowy_flutter`", "",
             "## Architecture", "",
             "* **UI layer (Flutter/Dart)** renders the document editor, sidebar, and settings dialogs.",
             "* **Rust core (`frontend/rust-lib`)** is compiled to native code and called via FFI.",
             "* **Event bus** carries typed requests/responses between Dart and Rust.", "",
             "## Key directories under `lib/`", ""]
    for d in dirs:
        lines.append(f"* `{d}/`")
    return "\n".join(lines) + "\n"


def regenerate_rust_backend():
    """Scan frontend/rust-lib/ for crate structure."""
    rust_dir = os.path.join(REPO_ROOT, "frontend", "rust-lib")
    if not os.path.isdir(rust_dir):
        return "# Rust Backend\n\n(rust-lib/ directory not found)\n"

    crates = sorted(d for d in os.listdir(rust_dir)
                    if os.path.isdir(os.path.join(rust_dir, d))
                    and os.path.exists(os.path.join(rust_dir, d, "Cargo.toml")))
    lines = ["# Rust Backend", "",
             "The core engine behind the AppFlowy client, written in Rust.", "",
             "## Location", "", "`frontend/rust-lib` (a Cargo workspace)", "",
             "## Crates", ""]
    for c in crates:
        lines.append(f"* `{c}`")
    return "\n".join(lines) + "\n"


def regenerate_agent_tools():
    """Scan appflowy-agent-tools/ for structure."""
    tools_dir = os.path.join(REPO_ROOT, "appflowy-agent-tools")
    items = sorted(os.listdir(tools_dir))
    lines = ["# Agent Tools (CLI + MCP)", "",
             "CLI and MCP server for driving the Tin deployment programmatically.", "",
             "## Contents", ""]
    for item in items:
        if item.startswith(".") or item == "node_modules":
            continue
        full = os.path.join(tools_dir, item)
        label = f"`{item}/`" if os.path.isdir(full) else f"`{item}`"
        lines.append(f"* {label}")
    return "\n".join(lines) + "\n"


def regenerate_static(path, title):
    """Preserve existing content for pages we don't auto-generate yet."""
    if os.path.exists(path):
        with open(path) as f:
            return f.read()
    return f"# {title}\n\n(waiting for regeneration logic)\n"


REGENERATORS = {
    "overview/Flutter Frontend.md": regenerate_flutter_frontend,
    "overview/Rust Backend.md": regenerate_rust_backend,
    "overview/Agent Tools (CLI + MCP).md": regenerate_agent_tools,
}


def main():
    updated = []
    for rel in sorted(os.listdir(os.path.join(CONTENT_DIR, "overview"))):
        path = os.path.join(CONTENT_DIR, "overview", rel)
        key = f"overview/{rel}"
        if key in REGENERATORS:
            content = REGENERATORS[key]()
            with open(path, "w") as f:
                f.write(content)
            updated.append(key)

    for container in ["operations", "confidential", "droid-automation"]:
        container_dir = os.path.join(CONTENT_DIR, container)
        if not os.path.isdir(container_dir):
            continue
        for rel in sorted(os.listdir(container_dir)):
            path = os.path.join(container_dir, rel)
            key = f"{container}/{rel}"
            # Later: add regenerators for operations/confidential/droid pages
            print(f"  (static) {key}")

    print(f"\nRegenerated {len(updated)} pages.")
    for u in updated:
        print(f"  {u}")
    print("\nRun: collab-sync sync --all --manifest manifest.json")


if __name__ == "__main__":
    main()
