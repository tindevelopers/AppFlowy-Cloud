#!/usr/bin/env bash
# Re-seed all 11 AppFlowy workspace pages with marker-bearing content.
# Run from: appflowy-agent-tools/collab-sync/

set -e
CONTENT="../importer/content"
BIN="target/debug/collab-sync"

seed() {
    local key="$1"
    local vid="$2"
    local md="${CONTENT}/${key}.md"
    echo "=== seed: $key ($vid) ==="
    $BIN seed "$vid" "$md" || echo "  FAILED for $key"
    echo ""
}

seed "overview/Flutter Frontend" "0c0b2e92-c9f7-4021-a6e3-d306e160490e"
seed "overview/Rust Backend" "b88f8e70-ac99-43ed-95bb-51abb61c001c"
seed "overview/Agent Tools (CLI + MCP)" "df2da82b-7b78-4a1b-af32-7e14a232e79d"
seed "operations/Build & Deploy" "a091d7e5-7281-4d66-bcb9-5c33e0230d93"
seed "operations/Dev Setup" "d374380f-99f4-470a-9ab5-8ce2f0c6f651"
seed "operations/White Label (Tin)" "df287853-7b51-4d04-b476-34d0221c6380"
seed "confidential/Credentials Index" "90c67c63-68e2-46a2-b5d7-5af2d2024b00"
seed "confidential/Deployment Endpoints" "b1fa4697-a071-46a0-aec9-e8e79d8ad3c6"
seed "confidential/Git Remotes & Access" "c8c462c8-7cf2-4746-9340-b207e6501bbc"
seed "confidential/Access & Rotation Procedures" "c1b116b0-e2f0-429f-97c9-3bc802afc95e"
seed "droid-automation/appflowy-doc-droid Design" "139f73c5-2bf6-436c-af01-22f4650a36b5"

echo "=== All seeding complete ==="
