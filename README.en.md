# TheIsle Overlay v2

[Tiếng Việt](README.md) · **English**

Map overlay for **The Isle: Evrima** (Gateway). Circular minimap pinned to the
game window · full map with POIs, waypoints and travel trails · bilingual
VI/EN interface · one-click install with auto-update.

## Install

Download `TheIsle Overlay_x.x.x_x64-setup.exe` from
[Releases](https://github.com/toantranct/theisle-overlay/releases) and run it.
On first launch the app downloads the map data (~3 MB) to your machine.

> Windows may show a SmartScreen warning because the installer is not
> code-signed. Click **More info → Run anyway**.

## Anti-cheat safety

The game runs kernel-level Easy Anti-Cheat. This app is safe because it
**never touches the game process**:

- Position comes only from the **clipboard**, when you press Tab → "Asset
  Location" in game yourself — the app just reads back what the game
  voluntarily hands over.
- Hotkeys use `RegisterHotKey` (Windows' cooperative API), **not** a keyboard
  hook.
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
