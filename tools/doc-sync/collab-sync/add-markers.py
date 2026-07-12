#!/usr/bin/env python3
"""Add <!-- BEGIN GENERATED:main --> / <!-- END GENERATED:main --> markers to
all AppFlowy workspace markdown files. Skips files that already have markers.

Pattern: wraps everything after the first heading line with markers,
leaving the heading as human-editable preamble.
"""

import os, sys

MARKER_BEGIN = "<!-- BEGIN GENERATED:main -->"
MARKER_END = "<!-- END GENERATED:main -->"
CONTENT_DIR = os.path.join(os.path.dirname(__file__), "..", "importer", "content")

FILES = [
    "overview/Flutter Frontend.md",
    "overview/Rust Backend.md",
    "overview/Agent Tools (CLI + MCP).md",
    "operations/Build & Deploy.md",
    "operations/Dev Setup.md",
    "operations/White Label (Tin).md",
    "confidential/Credentials Index.md",
    "confidential/Deployment Endpoints.md",
    "confidential/Git Remotes & Access.md",
    "confidential/Access & Rotation Procedures.md",
    "droid-automation/appflowy-doc-droid Design.md",
]

for rel in FILES:
    path = os.path.join(CONTENT_DIR, rel)
    with open(path, "r") as f:
        content = f.read()

    if "BEGIN GENERATED:" in content:
        print(f"SKIP {rel} (already has markers)")
        continue

    lines = content.splitlines()
    out = []
    found_heading = False
    inserted = False

    for i, line in enumerate(lines):
        out.append(line)
        if line.startswith("# ") and not found_heading:
            found_heading = True
        elif found_heading and not inserted:
            out.append("")
            out.append(MARKER_BEGIN)
            inserted = True

    out.append(MARKER_END)

    new_content = "\n".join(out) + "\n"
    with open(path, "w") as f:
        f.write(new_content)
    print(f"OK  {rel}")

print("\nDone. All files now have section markers.")
