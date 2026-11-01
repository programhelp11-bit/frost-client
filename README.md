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

1. **One-time, for the maintainer (users never see this):** register an Azure app at
   <https://portal.azure.com> -> *App registrations* -> *New registration*, account type
   *Personal Microsoft accounts only*, platform *Mobile and desktop applications* with redirect URI
   `http://localhost`, enable *Allow public client flows*. Then add the *Application (client) ID* as a repository
   **secret** `FROST_MS_CLIENT_ID` (GitHub -> Settings -> Secrets and variables -> Actions -> Secrets).
   The CI bakes it into the binary. For local builds: `FROST_MS_CLIENT_ID=<id> cargo run --release`.
   New Azure apps must also be approved for the Minecraft services API once (<https://aka.ms/AppRegInfo>).
   After that, players just click **Login** – the Microsoft sign-in opens in their browser and Frost catches the redirect.
2. Install a matching Java (8 for 1.8.9, 21 for 1.21.x, 25 for 26.x) – or set the path in *Settings*.

## Build

```bash
# Linux build deps: pkg-config libxkbcommon-dev libwayland-dev libx11-dev libgl1-mesa-dev libegl1-mesa-dev
cargo run --release
cargo test
```

Packaging (done by CI): `packaging/windows/frost.nsi` (NSIS) and `packaging/linux/build-appimage.sh`.
Tagging `vX.Y.Z` publishes both files as a GitHub release.

## v1.0.0 notes

* **Launcher**: Tauri 2 (HTML/CSS UI, bluish blur + snow). Pages: Home, Versions, Browse (Modrinth mods / resource packs / shaders installed straight into the instance), Cosmetics, HUD studio, Trainer, Settings.
* **In game**: press **Right Shift** to open the Frost menu — drag/scale/hide HUD widgets, spawn trainer bots, knockback and Bedwars bridge trainers, quality-of-life toggles. Edits are saved by the launcher.
* **Mods**: Fabric (1.21.10, 1.21.11, 26.1.2, 26.2) and Forge (1.8.9) builds are bundled; the launcher installs the loader, Fabric API and Java automatically.
* **Microsoft login** needs your own Azure application: set the repository **secret** `FROST_MS_CLIENT_ID` (Settings → Secrets and variables → Actions) to its *Application (client) ID* (a GUID, not a name) — see above — or paste it in Settings → Accounts.
* **Known limits**: the game-side features were compiled and unit-tested in CI but not played-tested; capes rendering, hitboxes and item physics are not implemented yet; the Frost cosmetics backend (`docs/BACKEND_API.md`) is not provided.
