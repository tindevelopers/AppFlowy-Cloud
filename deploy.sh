#!/bin/bash
set -e

# =============================================================================
# Tin / AppFlowy Cloud - Production Deploy Script for Hetzner VPS
# =============================================================================
# Usage: ./deploy.sh <domain> <server_ip> [ssh_key_path]
# Example: ./deploy.sh tin.example.com 123.456.789.0 ~/.ssh/id_rsa
# =============================================================================

DOMAIN=${1:-""}
SERVER_IP=${2:-""}
SSH_KEY=${3:-""}

if [ -z "$DOMAIN" ] || [ -z "$SERVER_IP" ]; then
  echo "Usage: ./deploy.sh <domain> <server_ip> [ssh_key_path]"
  echo "Example: ./deploy.sh tin.example.com 123.456.789.0"
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_APP_DIR="$(cd "$SCRIPT_DIR/../appflowy-web" && pwd)"

if [ ! -d "$WEB_APP_DIR" ]; then
  echo "Error: appflowy-web not found at $WEB_APP_DIR"
  exit 1
fi

echo "=== Building Tin web app for production ==="
echo "Domain: $DOMAIN"

cd "$WEB_APP_DIR"

# Backup existing .env
cp .env .env.local.backup 2>/dev/null || true

# Write production env for web app (must use APPFLOWY_ prefix - vite envPrefix)
cat > .env <<EOF
APPFLOWY_BASE_URL=https://$DOMAIN
APPFLOWY_GOTRUE_BASE_URL=https://$DOMAIN/gotrue
APPFLOWY_WS_BASE_URL=wss://$DOMAIN/ws/v2
EOF

npm run build

echo "=== Preparing server files ==="

# Create a temp deploy bundle
DEPLOY_BUNDLE=$(mktemp -d)
mkdir -p "$DEPLOY_BUNDLE/web"
cp -r "$WEB_APP_DIR/dist" "$DEPLOY_BUNDLE/web/"
cp "$SCRIPT_DIR/docker-compose.yml" "$DEPLOY_BUNDLE/"
cp "$SCRIPT_DIR/nginx/nginx.conf" "$DEPLOY_BUNDLE/"
cp "$SCRIPT_DIR/.env" "$DEPLOY_BUNDLE/"

# Generate self-signed SSL cert for initial setup
mkdir -p "$DEPLOY_BUNDLE/nginx/ssl"
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout "$DEPLOY_BUNDLE/nginx/ssl/private_key.key" \
  -out "$DEPLOY_BUNDLE/nginx/ssl/certificate.crt" \
  -subj "/C=US/ST=State/L=City/O=Tin/CN=$DOMAIN" 2>/dev/null || true

# Create production .env
sed -i.bak "s/^FQDN=.*/FQDN=$DOMAIN/" "$DEPLOY_BUNDLE/.env"
sed -i.bak "s/^SCHEME=.*/SCHEME=https/" "$DEPLOY_BUNDLE/.env"
sed -i.bak "s/^WS_SCHEME=.*/WS_SCHEME=wss/" "$DEPLOY_BUNDLE/.env"
rm -f "$DEPLOY_BUNDLE/.env.bak"

# Update admin_frontend base URL in docker-compose
sed -i.bak "s|APPFLOWY_GOTRUE_BASE_URL=.*|APPFLOWY_GOTRUE_BASE_URL=https://$DOMAIN/gotrue|" "$DEPLOY_BUNDLE/docker-compose.yml"
sed -i.bak "s|APPFLOWY_BASE_URL=.*|APPFLOWY_BASE_URL=https://$DOMAIN|" "$DEPLOY_BUNDLE/docker-compose.yml"
rm -f "$DEPLOY_BUNDLE/docker-compose.yml.bak"

SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null"
if [ -n "$SSH_KEY" ]; then
  SSH_OPTS="$SSH_OPTS -i $SSH_KEY"
fi

echo "=== Deploying to $SERVER_IP ==="

# Bootstrap server if needed
ssh $SSH_OPTS "root@$SERVER_IP" <<REMOTE
set -e
if ! command -v docker &> /dev/null; then
  echo "Installing Docker..."
  apt-get update
  apt-get install -y docker.io docker-compose-v2
  systemctl enable --now docker
fi

mkdir -p /opt/appflowy-cloud
REMOTE

# Copy files
rsync -avz --delete -e "ssh $SSH_OPTS" "$DEPLOY_BUNDLE/" "root@$SERVER_IP:/opt/appflowy-cloud/"

# Start services
ssh $SSH_OPTS "root@$SERVER_IP" <<REMOTE
cd /opt/appflowy-cloud

echo "Pulling latest Docker images..."
docker compose pull

echo "Starting services..."
docker compose up -d

echo "=== Deployment complete ==="
echo "Web app: https://$DOMAIN"
echo "Admin console: https://$DOMAIN/console"
echo ""
echo "NOTE: Using self-signed SSL certificate."
echo "For production, replace nginx/ssl/ with real certificates:"
echo "  - Let's Encrypt: certbot --nginx -d $DOMAIN"
echo "  - Or copy your own certificate.crt and private_key.key"
REMOTE

# Restore local web env
cd "$WEB_APP_DIR"
if [ -f .env.local.backup ]; then
  mv .env.local.backup .env
else
  rm -f .env
fi

# Cleanup
rm -rf "$DEPLOY_BUNDLE"

echo "=== All done ==="
echo "Your Tin instance is deployed at https://$DOMAIN"
