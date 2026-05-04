# Self-Hosting Wealthfolio

Wealthfolio ships an official Docker image so you can run the web edition on
your own hardware. Full self-hosting guides live on the website:

📘
**[wealthfolio.app/docs/guide/self-hosting](https://wealthfolio.app/docs/guide/self-hosting)**

This directory only holds in-repo artifacts (the Unraid CA template) and short
pointers per platform.

## Image

Multi-arch (`linux/amd64`, `linux/arm64`), published on every `v*.*.*` tag:

| Registry   | Image                               |
| ---------- | ----------------------------------- |
| Docker Hub | `afadil/wealthfolio:latest`         |
| GHCR       | `ghcr.io/afadil/wealthfolio:latest` |

```bash
docker pull afadil/wealthfolio:latest
```

## Platform pointers

- [**Docker / Docker Compose**](https://wealthfolio.app/docs/guide/self-hosting)
  — the canonical path. Full walkthrough on the website.
- [**Unraid**](./unraid/) — install via Community Apps. The CA template lives in
  this repo at [`unraid/template.xml`](./unraid/template.xml).
- [**Proxmox VE**](./proxmox/) — LXC via community-scripts, Docker-in-LXC, or
  Docker VM.
