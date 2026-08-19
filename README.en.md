# TheIsle Overlay v2

[Tiếng Việt](README.md) · **English**

Map overlay for **The Isle: Evrima** (Gateway). Circular minimap pinned to the
game window · full map with POIs, place names, waypoints and travel trails ·
your dino's stats from the server's IslePilot panel · bilingual VI/EN
interface · one-click install with auto-update.

## Features

- **Circular minimap** pinned to a corner of the game window, click-through so it
  never blocks play. North stays up, with an arrow showing your direction of travel.
- **Full map**: smooth zoom/pan, 9 toggleable layers (water, salt licks, mud wallows,
  sanctuaries, migration zones, AI patrol zones, food zones, region names, landmarks),
  with place names drawn directly on the map.
- **Waypoints**: right-click to drop, rename, delete; bearing and distance to the
  nearest one.
- **Travel trail** recorded per session, with the previous session's path restored.
- **Your dino**: growth, health, hunger, thirst and Prime progress read from the
  server's IslePilot panel, plus a compact stats strip under the minimap.
- **Global hotkeys** rebindable in-app, bilingual UI, automatic updates.

## Install

Download `TheIsle Overlay_x.x.x_x64-setup.exe` from
[Releases](https://github.com/toantranct/theisle-overlay/releases) and run it.
On first launch the app downloads the map data (~3 MB) to your machine.

Requires **Windows 10/11 64-bit**. WebView2 is already present on most Windows 11
installs; the installer fetches it if missing.

> Windows may show a SmartScreen warning because the installer is not
> code-signed. Click **More info → Run anyway**.

## How light is it?

Measured on a real machine: **Intel Core i5-14400F (10 cores / 16 threads), 32 GB
RAM, RTX 3060 Ti, Windows 11 Pro build 26200, 100% display scaling** — release
build v2.1.0:

| Item | Size |
|---|---|
| Installer | **4.3 MB** |
| Installed executable | 17.8 MB |
| Map data downloaded on first run | 2.9 MB (2.6 MB basemap + 0.3 MB point data) |
| **Total disk footprint** | **~21 MB** |

| At runtime | Measurement |
|---|---|
| RAM (full map **and** minimap open) | ~528 MB across 8 processes |
| RAM of the app process alone | 50 MB |
| Idle CPU (no interaction) | **0.3%** (≈5% of one thread out of 16) |

Why the RAM figure looks like that: the UI runs on WebView2 (Edge), which spawns
several helper processes, and the 3900×3908 px basemap occupies ~60 MB once
decoded. This is the **worst case** — while gaming you normally hide the full map
(`Ctrl+Alt+F`) and keep only the minimap, and Windows then trims the hidden
window's memory.

The app has **no repaint loop**: it draws only when new data arrives, so it costs
practically no CPU while you play.

## Things to know

1. **Game display mode**: no out-of-process overlay can draw over **Exclusive
   Fullscreen** — a Windows limitation. Use **Windowed** or **Borderless
   Fullscreen**. The app reads your game config and warns you if the mode is wrong.
2. **Position does not update by itself**: you press `Tab` → **Asset Location** in
   game whenever you want a position update. This is *deliberate* — see the
   anti-cheat section below.
3. **Heading needs two coordinate copies** at least 20 m apart; samples older than
   10 minutes expire so the arrow never points the wrong way.
4. **Only one instance can run** — global hotkeys are system-exclusive, so two
   copies would fight over them.
5. **Hotkeys taken by another app** are reported at startup; rebind them in Settings.
6. **The "Your dino" feature** only supports IslePilot-based servers
   (`xxx.islepilot.eu`). It reads data by parsing the server's web pages (there is
   no official API), so it **can break whenever IslePilot changes their markup** —
   the app flags it when it detects a new deployment. If this part fails, the map
   features are **unaffected**.
7. **Ask your server admins** before using it routinely — some servers have their
   own rules about third-party tools. Auto-position from the live map is OFF by default.
8. **Your panel session cookie** is encrypted with Windows DPAPI and can only be
   decrypted by your Windows account on that machine.
9. **SmartScreen** warns on first install because the installer is not code-signed
   (certificates cost a yearly fee). Later auto-updates are not prompted again.

## Anti-cheat safety

The game runs kernel-level Easy Anti-Cheat. This app is safe because it
**never touches the game process**:

- Position comes only from the **clipboard**, when you press Tab → "Asset
  Location" in game yourself — the app just reads back what the game
  voluntarily hands over.
- Hotkeys use `RegisterHotKey` (Windows' cooperative API), **not** a keyboard
  hook.
- Dino stats come over **HTTPS from the server's own website** (the IslePilot
  panel) — again, nothing to do with the game process.
- Never: reading game memory, DLL injection, DirectX hooks, synthetic input,
  packet capture, auto-copying coordinates on a timer, or sharing positions
  between players.

CI greps for any forbidden API call site (`scripts/check-forbidden-apis.ps1`).
The allowed-call list lives at the top of `src-tauri/src/win/mod.rs`.

## Development

Requirements: Node 22+, Rust stable (MSVC), WebView2.

```powershell
npm install
npx tauri dev                        # run dev

# Drive the UI without the game running:
$env:THEISLE_REPLAY = "path\to\replay_sample.txt"; npx tauri dev

# Tests
npm run check                        # svelte-check
cd src-tauri; cargo test --workspace # all Rust tests
cargo clippy --workspace -- -D warnings
..\scripts\check-forbidden-apis.ps1

# After each in-game map update:
cargo run --bin verify_data --features devtools
cargo test -p theisle-overlay --lib -- --ignored parse_real_cache
```

Note: `.cargo/config.toml` moves the `target-dir` outside the OneDrive-synced
folder.

## Releasing

1. Add repository secrets: `TAURI_SIGNING_PRIVATE_KEY` (content of
   `~/.tauri/theisle-overlay.key`) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
   (that key's password — kept out of the repo).
2. Bump `version` in `src-tauri/tauri.conf.json` and `package.json`.
3. `git tag v2.x.x && git push --tags` — the `release.yml` workflow builds the
   NSIS installer, signs the update artifact, generates `latest.json` and
   creates the GitHub Release.
4. Running apps see the new version and offer to update.

If the repo name/owner changes, edit `plugins.updater.endpoints` in
`src-tauri/tauri.conf.json`.

## Architecture

- `src-tauri/crates/overlay-core` — pure logic (coordinate parsing,
  world↔pixel transform, tracker) plus the complete test suite ported from
  the Python app. The frontend **never** computes a transform of its own;
  every payload carries both raw cm and pixels.
- `src-tauri/src` — Win32 (the safety boundary lives in `win/`), clipboard
  watcher, hotkeys, settings/store (identical paths and formats to the old
  Python app — existing users keep all their data), data fetch, minimap
  window management.
- `src-tauri/src/islepilot` — IslePilot panel integration (reads `/me` + `/map`,
  DPAPI-encrypted cookie); runs on its own thread so a failure there cannot take
  the map down.
- `src/main` — main window (Svelte 5 + Tailwind + Leaflet CRS.Simple).
- `src/minimap` — separate entry, plain canvas, no framework: a webview that
  runs beside the game for hours must stay minimal and only draws on events
  (0% idle CPU).

Map data is **fetched on first run, never bundled** — the basemap belongs to
VulnonaMAP (derived from Afterthought LLC game assets); a personal copy on
the user's machine is a different thing from the app redistributing that
data.

## Credits

- Basemap: [VulnonaMAP](https://vulnona.com/game/map/) (Coco.N) — stitched
  from in-game captures. Imagery copyright Afterthought LLC (The Isle).
- POIs: [myislemap.com](https://myislemap.com/), VulnonaMAP, wiredredman's
  Steam guide.

Unaffiliated with Afterthought LLC.
