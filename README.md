# ❄ Frost Client

Liquid-glass Minecraft PvP launcher and client runtime written in Rust (egui + tokio).
Windows `.exe` installer and Linux `.AppImage` are built by GitHub Actions.

Supported versions: **1.8.9**, **1.21.10**, **1.21.11**, **26.1.2**, **26.2**

## Status

| Area | State |
|------|-------|
| Launcher (login, versions, downloads, launch, skin/cape UI, HUD editor, trainer UI) | implemented in Rust |
| HUD model + renderer, bot AI, knockback maths, cosmetics sync, IPC bridge | implemented in Rust, unit-tested |
| `frost-agent.jar` (in-game Java agent: draws HUD, spawns bots, tab icon, cape rendering) | **not included yet** – protocol in `src/ipc.rs` |
| Frost backend (capes / presence) | **not included yet** – contract in `docs/BACKEND_API.md` |

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Setup

1. Register an Azure app (public client, device code flow, `XboxLive.signin`) and set its client id in *Settings* or `FROST_MS_CLIENT_ID`.
2. Install a matching Java (8 for 1.8.9, 21 for 1.21.x, 25 for 26.x) – or set the path in *Settings*.

## Build

```bash
# Linux build deps: pkg-config libxkbcommon-dev libwayland-dev libx11-dev libgl1-mesa-dev libegl1-mesa-dev
cargo run --release
cargo test
```

Packaging (done by CI): `packaging/windows/frost.nsi` (NSIS) and `packaging/linux/build-appimage.sh`.
Tagging `vX.Y.Z` publishes both files as a GitHub release.
