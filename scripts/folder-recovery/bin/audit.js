#!/usr/bin/env node
'use strict';

/*
 * af-folder-audit - Report workspaces whose af_folder_view read-model looks
 * out of sync with the Yjs Folder collab (source of truth).
 *
 *   PGURL=postgresql://...
 *   APPFLOWY_API_URL=https://projects.example.com
 *   APPFLOWY_TOKEN=<superadmin bearer>
 *   af-folder-audit [--limit N]
 *
 * The Yjs folder is fetched via the AppFlowy Cloud API
 * (GET /api/workspace/v1/{ws}/collab/{ws}?collab_type=3). This mirrors the
 * fetch path the desktop/web client uses, so it works on the Tin deployment
 * regardless of the on-disk af_collab.blob wire format.
 *
 * Exit code 0 = healthy; 2 = drift detected; 1 = fatal error.
 */

const { Client } = require('pg');
const { fetchFolderViaApi } = require('../lib/project');

const PGURL   = process.env.PGURL;
const APIURL  = process.env.APPFLOWY_API_URL;
const TOKEN   = process.env.APPFLOWY_TOKEN;
if (!PGURL)  { console.error('PGURL required'); process.exit(1); }
if (!APIURL) { console.error('APPFLOWY_API_URL required'); process.exit(1); }
if (!TOKEN)  { console.error('APPFLOWY_TOKEN required'); process.exit(1); }

const args  = process.argv.slice(2);
const limit = (() => {
  const i = args.indexOf('--limit');
  return i >= 0 ? parseInt(args[i + 1] || '0', 10) : 0;
})();

async function main() {
  const pg = new Client({ connectionString: PGURL });
  await pg.connect();
  let drift = 0;
  try {
    const q = limit > 0
      ? `SELECT workspace_id::text AS wsid FROM af_workspace ORDER BY workspace_id LIMIT ${limit}`
      : `SELECT workspace_id::text AS wsid FROM af_workspace ORDER BY workspace_id`;
    const wsRes = await pg.query(q);

    for (const { wsid } of wsRes.rows) {
      let folder;
      try { folder = await fetchFolderViaApi(APIURL, TOKEN, wsid); }
      catch (e) { console.error(`  ${wsid}: cannot fetch Yjs folder: ${e.message}`); continue; }
      if (!folder || !folder.views) continue;

      const yjsCount = Object.keys(folder.views).length;
      const rowRes = await pg.query(
        `SELECT count(*)::int AS n,
                bool_or(view_id = workspace_id AND parent_view_id IS NULL) AS has_root
         FROM af_folder_view WHERE workspace_id = $1::uuid`,
        [wsid],
      );
      const { n, has_root } = rowRes.rows[0];
      if (n !== yjsCount || !has_root) {
        drift += 1;
        console.log(`DRIFT ${wsid}  af_folder_view=${n}  yjs=${yjsCount}  has_root=${has_root === true}`);
      }
    }
    if (drift === 0) console.log('OK: no drift detected');
  } finally {
    await pg.end();
  }
  process.exit(drift === 0 ? 0 : 2);
}

main().catch(e => { console.error('FATAL:', e); process.exit(1); });
