# Wealthfolio on Proxmox VE

Three sensible install paths on Proxmox: a native LXC via the
[community-scripts](https://community-scripts.github.io/ProxmoxVE/) project,
Docker inside an LXC, or Docker inside a VM. The LXC path matches Proxmox
conventions (no Docker-in-LXC), but builds from source on each install (~15–25
min, tracked in [#563](https://github.com/wealthfolio/wealthfolio/issues/563)). The
Docker paths are faster but introduce nesting.

📘 **Full setup guide:**
[wealthfolio.app/docs/guide/self-hosting](https://wealthfolio.app/docs/guide/self-hosting)

## Getting started: LXC (recommended)

Open a shell on the **Proxmox host** (not inside an existing container) and run:

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/community-scripts/ProxmoxVE/main/ct/wealthfolio.sh)"
```

Defaults: Debian 13, 4 CPU / 4 GB RAM / 10 GB disk, port `8080`. Credentials are
written to `/root/wealthfolio.creds` inside the container.

## Getting started: Docker

If you already run a Docker host (LXC or VM) on Proxmox, just deploy the
container there like any other service. See the website guide above for the full
Compose walkthrough.
