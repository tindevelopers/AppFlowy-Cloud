# Folder Recovery (Tin fork)

Maintenance CLI for the Tin AppFlowy-Cloud deployment. Rebuilds the
`af_folder_view` read-model rows for one or more workspaces from the intact
Yjs Folder collab documents stored in `af_collab`.

## Background

The Tin deployment schema contains three tables that are not present in the
upstream `AppFlowy-IO/AppFlowy-Cloud` repository:

- `af_folder_view` (materialised folder tree, one row per view, served by the
  `GET /api/workspace/{wsid}/folder` endpoint).
- `af_folder_mutation_outbox` (Postgres-side transactional outbox, one row per
  workspace mutation).
- `af_folder_sync_state` (`last_synced_rid`, `is_full_projection`, `synced_at`
  per workspace; `last_synced_rid` uses Redis-stream-id format).

These tables are produced and consumed by the closed-source
`appflowyinc/appflowy_search` container that the Tin fork uses unchanged (see
`docker-compose.yml`, service `appflowy_search`). When that container is
unhealthy or has stopped consuming the outbox:

- New workspace creations or space edits may not be materialised into
  `af_folder_view`.
- The API returns `Record not found: There is no accessible folder view
  belonging to the root view id: <workspace_id>` for affected workspaces, and
  the desktop/web client shows a "Record not found" dialog with a
  `Workspace access denied` fallback.
- The Yjs folder collab in `af_collab` (partition_key = 3) is still intact and
  is the true source of truth.

## What this tool does

`af-folder-audit` scans every workspace, decodes the Yjs folder, and reports
those whose `af_folder_view` row count or root row do not match the Yjs tree.

`af-folder-rebuild` accepts one or more workspace UUIDs, reads the Yjs folder
from `af_collab`, projects it into `af_folder_view` rows (including
`prev_view_id`, `children[]`, `is_space`, icon/extra JSON), and writes the
result inside a per-workspace transaction. It also seeds or refreshes
`af_folder_sync_state` (`last_synced_rid = '0-0'`, `is_full_projection = true`)
so the row set is consistent with workspaces the search worker has processed.

Neither script writes to `af_collab` or `af_folder_mutation_outbox`; the Yjs
source of truth is untouched.

## When to use it

Use it as a stop-gap when the outbox consumer (`appflowy_search`) is stalled
and users are reporting a "Record not found" dialog on workspace open. It is
not a substitute for restoring the worker itself: check the search container's
health, its Postgres and Redis connectivity, and its logs first
(`docker compose logs appflowy_search`).

Typical order of operations:

1. Investigate the `appflowy_search` container and bring it back to a healthy
   state.
2. Run `af-folder-audit` to see which workspaces are still out of sync.
3. Run `af-folder-rebuild <wsid ...>` for each affected workspace.
4. Re-run `af-folder-audit`; expect `OK: no drift detected`.

## Installation and usage

The tool fetches the Yjs folder document via the same authenticated HTTP
endpoint the desktop and web clients use
(`GET /api/workspace/v1/{ws}/collab/{ws}?collab_type=3`). This avoids depending
on the on-disk `af_collab.blob` wire format, which the Tin deployment wraps
with an 8-byte prefix that upstream Yjs cannot decode directly.

```bash
cd scripts/folder-recovery
npm install

export PGURL='*********************************/<db>?sslmode=require'
export APPFLOWY_API_URL='https://projects.tinconnect.com'
export APPFLOWY_TOKEN='<superadmin bearer JWT>'

# List drifted workspaces (exit code 2 if any drift is detected)
node bin/audit.js
node bin/audit.js --limit 50   # optional throttle

# Rebuild specific workspaces
node bin/rebuild.js <workspace_id> [<workspace_id> ...]

# Preview without writing
node bin/rebuild.js --dry-run <workspace_id>
```

A superadmin JWT can be obtained from the browser session storage of a signed
in superadmin account (`localStorage.getItem('token')` -> `access_token`) or
via the GoTrue admin API. The token needs read access to every workspace
being audited or rebuilt.

Requires Node.js 20+.

## Safety notes

- Each workspace is rebuilt inside its own transaction. On any failure
  (e.g. row-count mismatch) the transaction rolls back and the next workspace
  is attempted.
- The rebuild refuses to run for a workspace whose Yjs folder does not contain
  a root view matching the workspace id, so it will not silently create an
  empty tree.
- Only `af_folder_view` and `af_folder_sync_state` are written. The outbox
  (`af_folder_mutation_outbox`) is left alone so a restored search worker can
  still process legitimate pending mutations.

## Non-goals

This tool does not:

- Restart or replace the `appflowy_search` container.
- Re-drive queued outbox entries. Entries whose target views no longer exist
  after a rebuild should be marked claimed with an explanatory
  `last_error` by operations, so a restored worker skips them.
- Rebuild any downstream index (search, embeddings) that `appflowy_search`
  maintains beyond `af_folder_view`.
