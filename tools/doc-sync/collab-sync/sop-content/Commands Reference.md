# Commands Reference

<!-- BEGIN GENERATED:main -->

All `collab-sync` commands. Run from `appflowy-agent-tools/collab-sync/`.

## list — View workspace hierarchy

```bash
cargo run -- list [workspace_id]
```

Prints the full page tree with view IDs. Defaults to the Source Code workspace. Pass any workspace UUID to explore a different workspace.

## fetch — Dump a page's block tree

```bash
cargo run -- fetch <view_id>
```

Downloads a page's Yjs document state, decodes it, and prints the block tree with block types, IDs, and text snippets. Also runs a marker scan to show available sections. **Read-only.**

## convert — Test MDImporter locally

```bash
cargo run -- convert [file.md]
```

Converts markdown to AppFlowy's internal `DocumentData` block tree and prints the result. No network calls. Useful for verifying how markdown will render before syncing.

## seed — Full page write (initial setup)

```bash
cargo run -- seed <view_id> <file.md> [--backup]
```

Writes the entire markdown file as a brand-new document. Use this for:
- First-time page creation
- Replacing all content on a page
- Pages that don't need section markers

Always pair `--backup` with seed when updating an existing page.

## sync — Section-based in-place update

```bash
cargo run -- sync <view_id> <file.md> --section <key> [--dry-run] [--backup]
cargo run -- sync --all --manifest <path> [--dry-run] [--backup]
```

Replaces only the content between matching `&lt;!-- BEGIN GENERATED:&lt;key&gt; --&gt;` and `&lt;!-- END GENERATED:&lt;key&gt; --&gt;` markers. Everything outside markers is left untouched.

- `--section <key>` — which section to replace (default: `main`)
- `--all` — process all pages listed in the manifest
- `--manifest <path>` — path to the JSON manifest (default: `manifest.json`)
- `--dry-run` — preview changes without writing
- `--backup` — save a pre-change snapshot to `artifacts/backups/`

## restore — Roll back from backup

```bash
cargo run -- restore <view_id> <backup_file>
```

Uploads a previously-saved `doc_state` backup to restore a page to an earlier state. Backups are stored in `artifacts/backups/<view_id>-<timestamp>.doc_state`.

## Manifest Format

```json
{
  "pages": {
    "page-key": {
      "view_id": "uuid",
      "source_md": "path/to/content.md",
      "sections": ["main", "sidebar"]
    }
  }
}
```

Each page can have multiple named sections. `source_md` paths are relative to the manifest location.
<!-- END GENERATED:main -->
