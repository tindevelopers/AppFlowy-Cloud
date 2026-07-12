#!/bin/bash
# Schedule: regenerate docs from codebase and sync to AppFlowy.
# Run from repo root. Intended for cron/CI.
#
# Usage:
#   ./appflowy-agent-tools/collab-sync/sync-pipeline.sh [--dry-run]

set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$DIR/../.." && pwd)"

cd "$REPO"

echo "=== Step 1: Regenerate docs ==="
python3 "$DIR/regenerate-docs.py"

echo "=== Step 2: Commit changes (if any) ==="
if ! git diff --quiet appflowy-agent-tools/importer/content/; then
    git add appflowy-agent-tools/importer/content/
    git diff --cached --stat
    echo "Changes detected. Commit and push for PR review, then run:"
    echo "  collab-sync sync --all --manifest collab-sync/manifest.json"
    exit 0
else
    echo "No content changes."
fi

echo "=== Step 3: Sync to AppFlowy ==="
cd "$DIR"
cargo run -- sync --all --manifest manifest.json --backup "$@"

echo "=== Done ==="
