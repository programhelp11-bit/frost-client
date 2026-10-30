# Frost Client – Architecture

## Directory tree

```
frost-client/
├── Cargo.toml
├── src/
│   ├── main.rs          # window + tokio runtime (2 worker threads)
│   ├── lib.rs
│   ├── config.rs        # Settings, data dir
│   ├── versions.rs      # Mojang manifest, rules, supported versions
│   ├── downloader.rs    # parallel, size+SHA-1 verified downloads
│   ├── auth.rs          # Microsoft device code -> XBL -> XSTS -> Minecraft
│   ├── launch.rs        # classpath, natives, JVM/game args, spawn
│   ├── cosmetics.rs     # universal skin, Frost capes, tab snowflake, cache
│   ├── hud_manager.rs   # HUD model, glass renderer, drag/resize editor, QoL settings
│   ├── trainer.rs       # bot brains (Sword/Crystal/Axe+Shield/Mace), knockback trainer
│   ├── ipc.rs           # loopback JSON-lines bridge launcher <-> in-game agent
│   ├── glass.rs         # liquid-glass theme, backdrop blur, snowflake painter
│   └── ui.rs            # launcher window (Play / Skins & Capes / HUD & QoL / Trainer / Settings)
├── packaging/
│   ├── windows/frost.nsi            # NSIS installer
│   └── linux/build-appimage.sh      # AppImage assembly
├── docs/
│   ├── ARCHITECTURE.md
│   └── BACKEND_API.md
└── .github/workflows/build.yml      # test + build + package + release
```

## Component diagram

```
 ┌──────────────────────── Frost launcher (Rust, this repo) ───────────────────────┐
 │ ui.rs ── auth.rs ── launch.rs ── downloader.rs ── versions.rs                   │
 │   │                                                                             │
 │   ├─ cosmetics.rs ── Mojang skin API (universal)                                │
 │   │               └─ Frost backend (capes, presence)  ◄── HTTPS                 │
 │   ├─ hud_manager.rs  (layout editor / preview)                                  │
 │   └─ ipc.rs  ◄───────── 127.0.0.1:25585 JSON lines ─────────┐                   │
 │        trainer.rs (bot brains run here)                      │                   │
 └──────────────────────────────────────────────────────────────┼───────────────────┘
                                                                │
                      Minecraft JVM  ── frost-agent.jar ────────┘
                      (Java agent / mod: reads HUD+QoL, draws HUD, spawns bot entities,
                       applies commands, adds ❄ to tab names, renders Frost capes)
```

## Rust vs. game side – what lives where

A Rust program cannot draw inside the Minecraft process. Frost therefore has two halves:

| Half | Language | Responsibility |
|------|----------|----------------|
| Launcher / runtime (this repo) | Rust | login, downloads, launch, settings, HUD layout + model, cosmetics sync, **all bot decision logic**, knockback maths |
| `frost-agent.jar` (**not part of this repo yet**) | Java (Fabric/Mixin per version, ASM for 1.8.9) | render HUD from [`hud_manager`] data, spawn/animate bot entities, QoL render hooks, tab-list prefix, cape rendering |

`launch.rs` adds `-javaagent:<data>/frost-agent.jar=port=…,uuid=…` automatically when that file exists.
The wire protocol is defined by the serde types in `ipc.rs` (`ToGame` / `FromGame`).

## Liquid-glass look

egui cannot blur pixels behind its window. The effect is layered:
1. transparent native window (`with_transparent`, `clear_color = 0`)
2. real backdrop blur from the OS: Acrylic via `window-vibrancy` on Windows 10/11; on Linux the compositor
   (KWin: *Window Rules → Blur*, Hyprland: `windowrulev2 = blur, class:^(frost-launcher)$`, picom blur rules)
3. translucent tints, upper sheen, specular edge highlight and thin white borders painted in `glass.rs`
   (the same helpers draw the in-game HUD widgets, so launcher and HUD match)

## Feature → implementation map

| Requirement | Where |
|-------------|-------|
| Versions 1.8.9 / 1.21.10 / 1.21.11 / 26.1.2 / 26.2 | `versions::SUPPORTED`, version JSON from Mojang (legacy + modern argument formats both handled in `launch.rs`) |
| Microsoft login | `auth.rs` (device code flow; needs your Azure client id) |
| RAM slider, JVM args | `ui.rs` Play/Settings tab -> `Settings` -> `launch::prepare` |
| Skin changer (visible to everyone) | `cosmetics::upload_skin` -> Mojang `POST /minecraft/profile/skins` |
| Cape uploader (Frost users only) | `cosmetics::FrostApi::upload_cape` + backend + agent rendering |
| Snowflake in tab list | presence registry (`/v1/presence`, `/v1/cosmetics/lookup`) -> `ToGame::Cosmetics` -> agent prefixes `cosmetics::TAB_PREFIX` |
| HUDs: CPS, FPS, Ping, Keystrokes, Armor, Potions, Direction, Reach | `hud_manager.rs` |
| Hitboxes, Fullbright, Minimal Viewmodel, Item Physics, Dynamic FOV, Zoom, Coords/Biome, FPS boost/chunk optimizer | `QolSettings` (UI toggles) – effects are applied by the agent |
| Movable / resizable HUD editor | `hud_manager::render_hud(.., edit = true, ..)` drag + scroll, grid snapping |
| Trainer bots (Sword, Crystal, Axe & Shield, Mace) | `trainer::BotBrain` |
| Knockback trainer | `trainer::apply_knockback`, `KbSession` |
| Low RAM/CPU, async | tokio with 2 workers, event-driven egui repaint, streaming SHA-1 (64 KiB buffer), `buffer_unordered(48)` asset downloads, `opt-level=3`, fat LTO, `panic=abort` |

## Security notes

* Microsoft/Mojang tokens never go to the Frost backend; it verifies ownership through Mojang's `join`/`hasJoined` handshake.
* `accounts.json` is written with mode 0600 on Unix. On Windows it lives in `%APPDATA%` (user-private); moving it to the OS credential store is a sensible hardening step.
* The IPC bridge listens on loopback only and rejects non-loopback peers. Add a per-launch shared secret before shipping to prevent other local processes from talking to it.
* Natives are extracted with `enclosed_name()` (zip-slip safe).
