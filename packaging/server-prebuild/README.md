# Wealthfolio Server — Linux amd64 prebuild

Standalone HTTP server build for self-hosting (no Tauri, no desktop runtime).
Built on Ubuntu 22.04 (glibc 2.35) — runs on Debian 12+, Ubuntu 22.04+, and
derivatives.

## Layout

- `wealthfolio-server` — server binary (install to `/usr/local/bin/`)
- `dist/` — frontend static assets (point `WF_STATIC_DIR` here)
- `wealthfolio.service.example` — sample systemd unit
- `LICENSE`

## Quick start

```bash
sudo install -m 755 wealthfolio-server /usr/local/bin/wealthfolio-server
sudo mkdir -p /opt/wealthfolio /opt/wealthfolio_data
sudo cp -r dist /opt/wealthfolio/dist

sudo tee /opt/wealthfolio/.env >/dev/null <<EOF
WF_LISTEN_ADDR=0.0.0.0:8080
WF_DB_PATH=/opt/wealthfolio_data/wealthfolio.db
WF_STATIC_DIR=/opt/wealthfolio/dist
WF_SECRET_KEY=$(openssl rand -base64 32)
WF_AUTH_PASSWORD_HASH=<argon2id hash, see docs/self-host>
EOF
sudo chmod 600 /opt/wealthfolio/.env

sudo cp wealthfolio.service.example /etc/systemd/system/wealthfolio.service
sudo systemctl enable --now wealthfolio
```

Full self-host docs:
<https://github.com/afadil/wealthfolio/blob/main/docs/self-host/>
