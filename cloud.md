# Cloud Deployment Guide

How to set up `browser-use-mcp` on a fresh VPS with a public Cloudflare tunnel.

## What this sets up

- **browser-use-mcp** — MCP HTTP server (port 7000) with live browser stream
- **Chrome CDP** — headless Chrome with remote debugging on localhost:9222
- **Xvfb** — virtual display on :99 (required for Chrome even in headless mode)
- **cloudflared** — Cloudflare quick tunnel exposing port 7000 publicly (no account needed)

---

## 1. System dependencies

```bash
apt-get update
apt-get install -y \
  curl wget git ffmpeg \
  xvfb \
  ca-certificates fonts-liberation libappindicator3-1 \
  libasound2 libatk-bridge2.0-0 libatk1.0-0 libcups2 \
  libdbus-1-3 libgdk-pixbuf2.0-0 libnspr4 libnss3 \
  libx11-xcb1 libxcomposite1 libxdamage1 libxrandr2 \
  libxss1 libxtst6 lsb-release xdg-utils
```

---

## 2. Install Google Chrome

```bash
wget https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb
dpkg -i google-chrome-stable_current_amd64.deb || apt-get install -f -y
rm google-chrome-stable_current_amd64.deb
```

---

## 3. Install cloudflared

```bash
curl -L https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64 \
  -o /usr/local/bin/cloudflared
chmod +x /usr/local/bin/cloudflared
```

---

## 4. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
```

---

## 5. Clone and build the server

```bash
mkdir -p /home/work
git clone https://github.com/omraval18/remote-browser-use-mcp /home/work/remote-browser-use-mcp
cd /home/work/remote-browser-use-mcp
cargo build -p browser-use-mcp --release
cp target/release/browser-use-mcp /usr/local/bin/browser-use-mcp
```

---

## 6. Create data directories

```bash
mkdir -p /home/work/browser-data/profiles
```

---

## 7. Systemd services

### Xvfb — virtual display

```bash
cat > /etc/systemd/system/xvfb-browser.service << 'EOF'
[Unit]
Description=Virtual display for browser-use-mcp
After=network.target

[Service]
Type=simple
ExecStartPre=/bin/sh -c 'rm -f /tmp/.X99-lock'
ExecStart=/usr/bin/Xvfb :99 -screen 0 1920x1080x24 -nolisten tcp
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
```

### Chrome CDP — headless Chrome on port 9222

```bash
cat > /etc/systemd/system/chrome-cdp.service << 'EOF'
[Unit]
Description=Chrome CDP
After=xvfb-browser.service

[Service]
ExecStart=/usr/bin/google-chrome-stable \
  --remote-debugging-port=9222 \
  --remote-debugging-address=127.0.0.1 \
  --headless=new \
  --no-sandbox \
  --disable-gpu
Restart=always
RestartSec=3
User=nobody

[Install]
WantedBy=multi-user.target
EOF
```

### browser-use-mcp — MCP server

Generate a strong random secret:
```bash
ADMIN_SECRET=$(openssl rand -hex 32)
echo "Save this: $ADMIN_SECRET"
```

```bash
cat > /etc/systemd/system/browser-use-mcp.service << EOF
[Unit]
Description=browser-use-mcp HTTP server (MCP + Admin API + live stream)
After=network.target chrome-cdp.service

[Service]
Type=simple
WorkingDirectory=/home/work/remote-browser-use-mcp
Environment=ADMIN_SECRET=${ADMIN_SECRET}
Environment=BROWSER_USE_PROFILES_DIR=/home/work/browser-data/profiles
Environment=BROWSER_USE_DB_PATH=/home/work/browser-data/mcp-users.db
Environment=DISPLAY=:99
ExecStart=/usr/local/bin/browser-use-mcp --http --port 7000 --host 0.0.0.0
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=browser-use-mcp

[Install]
WantedBy=multi-user.target
EOF
```

### cloudflared — public tunnel (no account needed)

```bash
cat > /etc/systemd/system/cloudflared-mcp.service << 'EOF'
[Unit]
Description=Cloudflare tunnel for browser-use-mcp
After=network.target browser-use-mcp.service

[Service]
Type=simple
ExecStart=/usr/local/bin/cloudflared tunnel --url http://localhost:7000 --no-autoupdate
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=cloudflared-mcp

[Install]
WantedBy=multi-user.target
EOF
```

---

## 8. Enable and start everything

```bash
systemctl daemon-reload
systemctl enable xvfb-browser chrome-cdp browser-use-mcp cloudflared-mcp
systemctl start xvfb-browser
systemctl start chrome-cdp
systemctl start browser-use-mcp
systemctl start cloudflared-mcp
```

Verify all four are running:
```bash
systemctl status xvfb-browser chrome-cdp browser-use-mcp cloudflared-mcp --no-pager
```

---

## 9. Get your public tunnel URL

```bash
journalctl -u cloudflared-mcp -n 50 | grep -i "trycloudflare\|https://"
```

The URL looks like `https://some-words.trycloudflare.com`. It changes every time cloudflared restarts.

---

## 10. Create a user and get their API key

```bash
TUNNEL_URL="https://your-tunnel-url.trycloudflare.com"

# Create a user
curl -X POST "$TUNNEL_URL/api/users" \
  -H "Authorization: Bearer $ADMIN_SECRET" \
  -H "Content-Type: application/json" \
  -d '{"user_id": "alice"}'

# Rotate / get their key (returns plaintext key)
curl -X POST "$TUNNEL_URL/api/users/alice/rotate-key" \
  -H "Authorization: Bearer $ADMIN_SECRET"
```

---

## 11. Live browser view

Open in your browser — no install needed:
```
https://your-tunnel-url.trycloudflare.com/view/alice?key=ADMIN_SECRET
```

Tabs:
- **Browser Tab** — Chrome's page content via CDP `captureScreenshot` (~10-20 fps)
- **Desktop** — full Xvfb display via ffmpeg MJPEG (requires ffmpeg, headless server shows Xvfb desktop)

---

## 12. Redeploying after code changes

```bash
cd /home/work/remote-browser-use-mcp
git pull
source ~/.cargo/env
cargo build -p browser-use-mcp --release
systemctl stop browser-use-mcp
cp target/release/browser-use-mcp /usr/local/bin/browser-use-mcp
systemctl start browser-use-mcp
```

---

## Useful commands

```bash
# Tail live logs
journalctl -u browser-use-mcp -f

# Check Chrome is reachable
curl http://localhost:9222/json/version

# List users
curl http://localhost:7000/api/users \
  -H "Authorization: Bearer $ADMIN_SECRET"

# Revoke a user
curl -X DELETE http://localhost:7000/api/users/alice \
  -H "Authorization: Bearer $ADMIN_SECRET"
```
