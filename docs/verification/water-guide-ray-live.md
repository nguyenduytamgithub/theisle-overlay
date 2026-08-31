# Water Guide Ray v1.9.0 — Verification Record

- Feature branch: `codex/water-guide-ray-v1`
- Design commit: `af0b015`
- Plan commit: `2a25f28`
- Status: `PARTIAL — source/package/install gates passed; live route/ray acceptance is still open`
- Safety boundary: player-controlled 2D guidance only; no memory access, injection, packet capture, synthetic input, character control, or terrain avoidance.

## Installed data snapshot used for design and integration

| Evidence | Value |
|---|---|
| POI path | `%LOCALAPPDATA%\TheIsleOverlay\pois_gateway.json` |
| Map/schema | `Gateway_v0.21.7` / `4` |
| Water POIs | `27` |
| POI SHA-256 | `22A5119C91E305A3FACBAA9F60244516B00115A267A9E7D65DA6EC0E8E7CFFBF` |
| Freshwater path | `%LOCALAPPDATA%\TheIsleOverlay\basemap\islemaps\freshwater.png` |
| Mask geometry | `2500 x 2500 RGBA` |
| Mask SHA-256 | `6C416181C818C46912C8345C0E183990915B6E7489FDDD06D22D71A79130E05A` |
| Opaque freshwater pixels | `178,368 (2.8539%)` |

Hashes identify this installed snapshot; a newer valid map must be revalidated, not rejected only because its hash differs.

## Automated evidence recorded before packaging

| Gate | Evidence | Result |
|---|---|---|
| Freshwater geometry RED/GREEN | Missing module/functions failed first; then 7 selector/validation tests passed | `PASS` |
| Route lock RED/GREEN | Missing runtime/state failed first; then 10 Water Guide Rust tests passed | `PASS` |
| Waiting auto-lock RED/GREEN | The new test first failed because `lock_waiting_with_position` did not exist; `waiting_request_locks_once_when_first_valid_position_arrives` now passes | `PASS` |
| Independent code review | Initial review found the activation freshness mismatch; follow-up commit `2c12516` uses payload `staleAfterS`, proves the exact boundary, rejects malformed thresholds, and the re-review returned no remaining findings | `READY` |
| Full Rust regression after waiting auto-lock | `203 passed, 9 ignored, 0 failed` across the workspace/all targets | `PASS` |
| Fixed-route geometry RED/GREEN | Missing module failed first; 80 m mutation produced 27,900 cm instead of 28,000 cm; restored suite passed | `PASS` |
| Navigation regression | `46 passed, 0 failed` | `PASS` |
| Svelte/TypeScript | `0 errors, 0 warnings` | `PASS` |
| Vite build | Emitted `dist/water-guide.html` and dedicated CSS/JS bundles | `PASS` |
| Window policy | `4 passed, 0 failed` for visibility, full-client geometry, capability, and recovery | `PASS` |

## Package, install, and live acceptance

| Gate | Evidence | Result |
|---|---|---|
| Scoped final verification matrix | 46 Node tests; Svelte 0/0; Vite build; changed-file rustfmt; Rust 203 passed/9 ignored; clippy `-D warnings`; `git diff --check` | `PASS` |
| Repository-wide rustfmt | The project-wide command exits 1 on unrelated pre-existing files; both changed Rust files pass `rustfmt --check` | `WAIVED TO CHANGED FILES` |
| NSIS installer | `D:\CodexBuild\theisle-overlay-water-guide-target\release\bundle\nsis\TheIsle Overlay_1.9.0_x64-setup.exe`; 6,013,794 bytes; SHA-256 `9F9FB8DB42AB60C4D585BFBAB6FB3A3D41EBE1423F3BBB198173CF745B82A2C0` | `PASS` |
| Installed executable | `%LOCALAPPDATA%\TheIsle Overlay\theisle-overlay.exe`; version `1.9.0`; 22,162,944 bytes; SHA-256 `C441CB99976A600507900CC7CEF932ADC643921FEAF5148318CD6A3C35F1F554` | `PASS` |
| The Isle process preserved while Overlay restarts | The Isle PID `22196` before and after silent current-user installation; only Overlay was stopped/restarted | `PASS` |
| Locked destination label/pixel/world coordinate | No activated-route log was captured. Independent replay from the real visible position `(Lat 15,740, Long -1,131)` selects `Jungle Pond`, mask `[1289,1234]`, world `[-8650.8,76497.2]` cm, `813.7` m | `PARTIAL` |
| Independent freshwater check | Pixel `[1289,1234]` alpha `255`, 8/8 water neighbours, exact round-trip `[1289.5,1234.5]`, selected from `17,204` inset boundary candidates | `PASS FOR REPLAY; NOT LIVE ACTIVATION` |
| `Ctrl+Alt+W` toggles in installed build | Installed build displayed `WATER GUIDE — CHỜ VỊ TRÍ` over the real game; the reproduced first-position race was fixed so the next valid server position auto-locks once | `PARTIAL` |
| Blue ray and repeated arrows visible | Deterministic renderer/geometry tests pass, but no screenshot of the ray over an active route was obtained | `BLOCKED LIVE VISUAL` |
| U-turn/heading-unknown/waiting states truthful | Waiting state captured over the real game; U-turn/ray states covered deterministically without character control | `PARTIAL` |
| Alt-Tab hides the guide | Window-policy test passes; no independent real-game screenshot pair captured | `PARTIAL` |
| Player reaches and drinks at destination | User-controlled walk/drink only | `PENDING USER CONTROLLED ACCEPTANCE` |

## Live evidence files

| Evidence | SHA-256 | What it proves |
|---|---|---|
| `D:\CodexBuild\desktop-dryo-spawn-next-20260831.png` | `7EDCBCCB47A1F6545E7E9FE4318C7832D9C268EB2DCDF3A6F2B429E317518DBF` | Installed Overlay displayed the fail-closed waiting state over an active Dryosaurus session on SDVN #2. |
| `D:\CodexBuild\desktop-game-tab-asset-location-20260831.png` | `4B9F4EB65EC72D188D3D765D5D4A7ED4216D62F52400F786094B94290C956A4D` | The real Status Report visibly showed `(Lat 15,740, Long -1,131)`, server `[SEA/VN]-SDVN-#2-No Rules`, and 63/300 players. |

The live session later produced fresh `source=server` position confirmations, but the guide had already been toggled off while uncovering the game's Asset Location field. The server then returned the client to the title map before a route screenshot was captured. This is not counted as a visual pass.

## Reproduced defect and fix

If `Ctrl+Alt+W` was pressed before a fresh position existed, the request stayed in `waiting_for_position` even when the next valid server/clipboard position arrived. The pipeline now offers every accepted `PositionUpdate` to `lock_waiting_from_position`; the runtime selects exactly once only when the request is waiting, keeps the fixed start/target afterward, and publishes the resulting route. It uses the payload's `staleAfterS` threshold (12 seconds in the current pipeline), rejects malformed freshness values, and accepts the exact boundary without admitting an older sample. Regression tests prove a later position cannot silently retarget an already locked route.

## Data-selection acceptance rule

The activated `targetMaskPx` must have alpha >= 128 in the same `freshwater.png`, lie on an inset freshwater boundary candidate, round-trip through `Calibration::islemaps()` to the route world coordinate, and use the nearest water POI only as its label. Failure of any check is `BLOCKED` and the guide must draw no ray.
