# Installation

<!-- BEGIN GENERATED:main -->

How to install and configure the collab-sync tool for AppFlowy workspace documentation management.

## Prerequisites

- **Node.js 18+** — for the MCP server
- **Rust 1.96+** — to compile the collab-sync binary
- **AppFlowy project** at `~/projects/AppFlowy` (or set `APPFLOWY_ROOT` env var)
- **`.env`** file at `appflowy-agent-tools/.env` with:

```
APPFLOWY_EMAIL=your-email
APPFLOWY_PASSWORD=your-password
APPFLOWY_BASE_URL=https://projects.tinconnect.com
```

## Quick Install

```bash
# Clone the factory-droids repo (if not already done)
git clone git@github.com:tindevelopers/factory-droids.git ~/factory-droids

# Install MCP server dependencies
cd ~/factory-droids/mcp-servers/appflowy-collab-sync && npm install

# First build of the Rust binary (automatic on first MCP tool call)
cd ~/projects/AppFlowy/appflowy-agent-tools/collab-sync && cargo build
```

## Register with Your IDE

### Factory Droid

```bash
droid mcp add appflowy-collab-sync -- \
  node ~/factory-droids/mcp-servers/appflowy-collab-sync/index.js
```

The `appflowy-collab-sync-operator` droid is also available for Factory users who prefer a droid over MCP tools.

### Cursor

Add to `.cursor/mcp.json` in your project:

```json
{
  "mcpServers": {
    "appflowy-collab-sync": {
      "command": "node",
      "args": ["~/factory-droids/mcp-servers/appflowy-collab-sync/index.js"],
      "env": { "APPFLOWY_ROOT": "/path/to/AppFlowy" }
    }
  }
}
```

### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "appflowy-collab-sync": {
      "command": "node",
      "args": ["~/factory-droids/mcp-servers/appflowy-collab-sync/index.js"],
      "env": { "APPFLOWY_ROOT": "/Users/foo/projects/AppFlowy" }
    }
  }
}
```

### VS Code / Cline / Other MCP Clients

Any MCP-compatible client can use this server. Point the client at:

```
command: node
args: ["~/factory-droids/mcp-servers/appflowy-collab-sync/index.js"]
```

Set `APPFLOWY_ROOT` if your AppFlowy project is not at `~/projects/AppFlowy`.

## First Sync

After registration, test that everything works:

```bash
# From any IDE prompt:
"List the Source Code workspace"
"Sync the SOP workspace with --dry-run"
```

Or from the terminal:

```bash
cd ~/projects/AppFlowy/appflowy-agent-tools/collab-sync
cargo run -- list
cargo run -- sync --all --manifest manifest.json --dry-run
```

## Updating

```bash
# Pull latest code
cd ~/factory-droids && git pull
cd ~/factory-droids/mcp-servers/appflowy-collab-sync && npm install

# Rebuild Rust binary if collab-sync code changed
cd ~/projects/AppFlowy/appflowy-agent-tools/collab-sync && cargo build
```

## Hetzner AppFlowy Cloud Deployment

The production Tin AppFlowy Cloud instance runs on a Hetzner VPS at `167.233.80.78`.

### Production environment

Edit `/opt/appflowy-cloud/.env` on the VPS and set the public domain:

```bash
FQDN=projects.tinconnect.com
SCHEME=https
WS_SCHEME=wss
APPFLOWY_BASE_URL=${SCHEME}://${FQDN}
APPFLOWY_WEBSOCKET_BASE_URL=${WS_SCHEME}://${FQDN}/ws/v2
APPFLOWY_WEB_URL=${APPFLOWY_BASE_URL}
```

Email must use the correct Brevo account:

```bash
APPFLOWY_MAILER_SMTP_HOST=smtp-relay.brevo.com
APPFLOWY_MAILER_SMTP_PORT=587
APPFLOWY_MAILER_SMTP_USERNAME=929d17001@smtp-brevo.com
APPFLOWY_MAILER_SMTP_EMAIL=projects@notify.tinconnect.com
APPFLOWY_MAILER_SMTP_PASSWORD=<brevo-smtp-key>
APPFLOWY_MAILER_SMTP_TLS_KIND=required

GOTRUE_SMTP_HOST=smtp-relay.brevo.com
GOTRUE_SMTP_PORT=587
GOTRUE_SMTP_USER=929d17001@smtp-brevo.com
GOTRUE_SMTP_ADMIN_EMAIL=projects@notify.tinconnect.com
GOTRUE_SMTP_PASS=<brevo-smtp-key>
```

### Restart services

After changing `.env`, recreate the affected containers:

```bash
ssh root@167.233.80.78
cd /opt/appflowy-cloud
docker compose up -d --force-recreate appflowy_cloud gotrue nginx
```

### Verification

- `GET https://projects.tinconnect.com/api/server-info` should return `appflowy_web_url: "https://projects.tinconnect.com"`.
- Workspace invite emails should be sent from `projects@notify.tinconnect.com`.
- Invite links should resolve to `https://projects.tinconnect.com/accept-invitation?invited_id=...`.

## Troubleshooting

| Problem | Fix |
|---------|-----|
| "APPFLOWY_EMAIL missing" | Create `.env` at `appflowy-agent-tools/.env` with credentials |
| "create_collab failed" | Ensure `cargo build` completed — the binary needs brotli support |
| MCP tools not appearing | Restart the IDE after adding the MCP config |
| "No such file" for node_modules | Run `npm install` in the MCP server directory |
| Binary not found at ~/projects | Set `APPFLOWY_ROOT` env var to your AppFlowy project path |
| Invite link points to `localhost` | Update `FQDN`, `SCHEME`, and `WS_SCHEME` in `/opt/appflowy-cloud/.env` and recreate containers |
| Invite email not delivered | Verify `APPFLOWY_MAILER_SMTP_EMAIL` and `GOTRUE_SMTP_*` use the `929d17001` Brevo account |
| "Seat limit reached" when inviting | AppFlowy Cloud license allows 1 Member/Owner seat; invite as Guest or upgrade license |
<!-- END GENERATED:main -->
