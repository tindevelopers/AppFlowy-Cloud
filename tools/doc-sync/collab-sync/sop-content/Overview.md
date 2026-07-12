# Collab Sync — Overview

<!-- BEGIN GENERATED:main -->

The `collab-sync` tool keeps AppFlowy workspace pages in sync with markdown files in your git repository. It updates pages **in place** (preserving view IDs, comments, and human edits) by replacing only the content inside section markers.

## Quick Start

```bash
cd appflowy-agent-tools/collab-sync

# Preview what would change (safe, no writes)
cargo run -- sync --all --manifest manifest.json --dry-run

# Sync all pages for real (with backup)
cargo run -- sync --all --manifest manifest.json --backup

# Sync a single page
cargo run -- sync <view_id> <file.md> --section main --backup
```

## Key Design Decisions

- **Section markers** separate generated content from human-editable content. The tool only touches what's between `&lt;!-- BEGIN GENERATED:&lt;key&gt; --&gt;` and `&lt;!-- END GENERATED:&lt;key&gt; --&gt;`.
- **Source of truth** is markdown in the git repo. Docs are regenerated from code, reviewed in PRs, then synced to AppFlowy.
- **Backup by default.** Always use `--backup` to save a pre-change snapshot. Restore with `cargo run -- restore <view_id> <backup_file>`.
- **In-place updates.** Pages keep their view IDs. Links, comments, and sidebar structure are never disrupted.

## Prerequisites

- Rust toolchain (1.96+)
- Access to the Tin AppFlowy Cloud at `projects.tinconnect.com`
- `.env` file at `appflowy-agent-tools/.env` with `APPFLOWY_EMAIL`, `APPFLOWY_PASSWORD`, `APPFLOWY_BASE_URL`

## Architecture

```
git repo markdown (source of truth)
        │  regenerated from code via regenerate-docs.py
        ▼
collab-sync fetch ──► Document::open ──► get_document_data()
        │
        ▼
MDImporter::import(md) ──► new DocumentData
        │
        ▼
merge_replacement() ──► replace marked region in block tree
        │
        ▼
Document::create() ──► encode_collab() ──► create_collab(upload)
```
<!-- END GENERATED:main -->
