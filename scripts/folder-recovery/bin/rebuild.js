#!/usr/bin/env node
'use strict';

/*
 * af-folder-rebuild - Rebuild af_folder_view rows for one or more workspaces
 * from the intact Yjs Folder collab document.
 *
 *   PGURL=postgresql://... af-folder-rebuild <workspace_id...> [--dry-run]
 *
 * Behaviour: per workspace, opens a transaction, deletes the existing
 * af_folder_view rows for that workspace, inserts the projection, and updates
 * (or seeds) af_folder_sync_state with last_synced_rid='0-0' and
 * is_full_projection=true. Rolls back on any error.
 *
 * Intended use: recover from a stalled appflowy_search outbox consumer. Not a
 * substitute for restoring the worker; run once after fixing the worker
 * (or as a stop-gap while operations diagnose it).
 */

const { Client } = require('pg');
const { fetchFolderViaApi, projectFolder } = require('../lib/project');

const args   = process.argv.slice(2);
const dryRun = args.includes('--dry-run');
const wsids  = args.filter(a => !a.startsWith('--'));
const PGURL  = process.env.PGURL;
const APIURL = process.env.APPFLOWY_API_URL;
const TOKEN  = process.env.APPFLOWY_TOKEN;

if (!PGURL)  { console.error('PGURL required'); process.exit(1); }
if (!APIURL) { console.error('APPFLOWY_API_URL required'); process.exit(1); }
if (!TOKEN)  { console.error('APPFLOWY_TOKEN required'); process.exit(1); }
if (!wsids.length) { console.error('at least one workspace id required'); process.exit(1); }

const INSERT_SQL = `INSERT INTO af_folder_view (
  workspace_id, view_id, parent_view_id, name, icon, layout, is_space, extra,
  created_at, created_by, last_edited_time, last_edited_by,
  prev_view_id, children, relation_hash, data_hash, updated_at
) VALUES (
  $1::uuid, $2::uuid, $3::uuid, $4, $5::jsonb, $6, $7, $8::jsonb,
  $9, $10, $11, $12,
  $13::uuid, $14::uuid[], '', '', now()
)`;

const UPSERT_SYNC_SQL = `
  INSERT INTO af_folder_sync_state (workspace_id, last_synced_rid, is_full_projection, synced_at, snapshot)
  VALUES ($1::uuid, '0-0', true, now(), NULL)
  ON CONFLICT (workspace_id) DO UPDATE
  SET last_synced_rid = EXCLUDED.last_synced_rid,
      is_full_projection = EXCLUDED.is_full_projection,
      synced_at = EXCLUDED.synced_at`;

async function rebuildOne(pg, wsid) {
  const folder = await fetchFolderViaApi(APIURL, TOKEN, wsid);
  if (!folder) throw new Error(`no folder document returned for ${wsid}`);
  const rows = projectFolder(wsid, folder);
  if (!rows.some(r => r.parent_view_id === null && r.view_id === wsid)) {
    throw new Error(`refusing to rebuild ${wsid}: Yjs folder has no root view matching workspace_id`);
  }

  if (dryRun) {
    console.log(`  [dry-run] would replace ${rows.length} rows for ${wsid}`);
    return;
  }

  await pg.query('BEGIN');
  try {
    const del = await pg.query(
      `DELETE FROM af_folder_view WHERE workspace_id = $1::uuid`, [wsid]
    );
    for (const r of rows) {
      await pg.query(INSERT_SQL, [
        r.workspace_id, r.view_id, r.parent_view_id, r.name,
        r.icon ? JSON.stringify(r.icon) : null,
        r.layout, r.is_space,
        r.extra ? JSON.stringify(r.extra) : null,
        r.created_at, r.created_by, r.last_edited_time, r.last_edited_by,
        r.prev_view_id, r.children,
      ]);
    }
    await pg.query(UPSERT_SYNC_SQL, [wsid]);
    const check = await pg.query(
      `SELECT count(*)::int AS n FROM af_folder_view WHERE workspace_id = $1::uuid`,
      [wsid],
    );
    if (check.rows[0].n !== rows.length) {
      throw new Error(`row count mismatch: expected ${rows.length}, got ${check.rows[0].n}`);
    }
    await pg.query('COMMIT');
    console.log(`  OK: deleted ${del.rowCount}, inserted ${rows.length} for ${wsid}`);
  } catch (e) {
    await pg.query('ROLLBACK');
    throw e;
  }
}

async function main() {
  const pg = new Client({ connectionString: PGURL });
  await pg.connect();
  try {
    for (const wsid of wsids) {
      console.log(`=== ${wsid} ===`);
      try { await rebuildOne(pg, wsid); }
      catch (e) { console.error(`  FAILED: ${e.message}`); process.exitCode = 2; }
    }
  } finally {
    await pg.end();
  }
}

main().catch(e => { console.error('FATAL:', e); process.exit(1); });
