# Troubleshooting

<!-- BEGIN GENERATED:main -->

Common issues and their fixes when using collab-sync.

## create_collab fails with "expected value at line 1 column 1"

**Symptom:** The `seed` or `sync` command fails with a JSON parse error from the server.

**Cause:** The `client-api` crate was compiled without brotli compression support. The AppFlowy Cloud API requires brotli-compressed protobuf payloads.

**Fix:** Ensure `Cargo.toml` has `features = ["enable_brotli"]` on the `client-api` dependency:

```toml
client-api = { git = "...", features = ["enable_brotli"] }
```

## "section 'X' not found on this page"

**Symptom:** `sync` exits with an error listing available sections but not finding your requested key.

**Causes:**
1. The page doesn't have markers yet. Run `seed` first to populate it with marker-bearing content.
2. The section key is spelled wrong. Check the marker scan output with `fetch <view_id>`.
3. The markers were removed by a human edit. Re-add them via the AppFlowy editor or re-seed.

## Content duplicated after seed

**Symptom:** The page shows every block twice.

**Cause:** The old content wasn't properly replaced. The `seed` command uses `create_collab` which the server should merge. If the page already has content, use `sync --section main` instead of `seed`.

**Fix:** Re-seed the page (the second write should overwrite the duplicate).

## Rust compilation errors about collab-entity

**Symptom:** Cargo can't find `collab-entity` or similar dependencies.

**Cause:** Missing `[patch.crates-io]` section in `Cargo.toml`. The collab crates are published on crates.io but the project uses a specific git revision.

**Fix:** Ensure the `[patch.crates-io]` section in `collab-sync/Cargo.toml` mirrors the entries in `frontend/rust-lib/Cargo.toml`.

## Dry-run works but real sync doesn't change the page

**Symptom:** `--dry-run` shows changes but after running without `--dry-run`, the browser shows old content.

**Cause:** The browser may have cached the old content.

**Fix:** Hard-refresh the page (Cmd+Shift+R) or close and reopen the workspace. The data was written; the browser is showing a stale view.

## Backup restoration fails

**Symptom:** `restore` command fails with "opening backup collab" error.

**Causes:**
1. The backup file is corrupted or incomplete. Check file size — it should be several KB.
2. The backup was taken with a different version of the collab crates. Use the same revision.

**Prevention:** Always verify backups immediately after creation by checking the file size.

## Getting the view ID for a page

Use the `list` command:

```bash
cargo run -- list [workspace_id]
```

This prints every page in the workspace with its UUID. The format is:

```
<view_id>  <indented name>
```

## Environment not configured

**Symptom:** "APPFLOWY_EMAIL missing" or "GoTrue sign-in failed".

**Fix:** Ensure `appflowy-agent-tools/.env` contains:

```
APPFLOWY_EMAIL=your-email
APPFLOWY_PASSWORD=your-password
APPFLOWY_BASE_URL=https://projects.tinconnect.com
```
<!-- END GENERATED:main -->
