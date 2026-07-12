---
name: AppFlowy Doc Sync
schedule: 0 7 * * *
---

# Heartbeat

Regenerate the AppFlowy workspace documentation and sync to the Tin cloud at projects.tinconnect.com.

Run the pipeline:
1. cd /Users/foo/projects/AppFlowy/appflowy-agent-tools/collab-sync
2. cargo build
3. python3 regenerate-docs.py
4. If any markdown files in ../importer/content/ changed: git add, commit, and push. Then run: cargo run -- sync --all --manifest manifest.json --backup
5. If no content changes, run: cargo run -- sync --all --manifest manifest.json --backup (the page content may still differ from what's on the server)
6. Report summary: pages synced, block counts, any failures.
