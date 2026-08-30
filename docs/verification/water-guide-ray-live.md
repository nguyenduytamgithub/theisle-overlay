# Water Guide Ray v1.9.0 — Verification Record

- Feature branch: `codex/water-guide-ray-v1`
- Design commit: `af0b015`
- Plan commit: `2a25f28`
- Status: `IN PROGRESS — automated source gates passed; package/install/live gates not run yet`
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
| Full Rust regression after route state | `140 passed, 9 ignored, 0 failed` in main library plus all workspace suites | `PASS` |
| Fixed-route geometry RED/GREEN | Missing module failed first; 80 m mutation produced 27,900 cm instead of 28,000 cm; restored suite passed | `PASS` |
| Navigation regression | `46 passed, 0 failed` | `PASS` |
| Svelte/TypeScript | `0 errors, 0 warnings` | `PASS` |
| Vite build | Emitted `dist/water-guide.html` and dedicated CSS/JS bundles | `PASS` |
| Window policy | `4 passed, 0 failed` for visibility, full-client geometry, capability, and recovery | `PASS` |

## Package, install, and live acceptance

| Gate | Evidence | Result |
|---|---|---|
| Full final verification matrix | Run after documentation/version sync | `PENDING LIVE RUN` |
| NSIS installer path, size, SHA-256 | Run after final matrix | `PENDING LIVE RUN` |
| Installed executable path, version, SHA-256 | Record after current-user installation | `PENDING LIVE RUN` |
| The Isle process preserved while Overlay restarts | Record before/after PIDs | `PENDING LIVE RUN` |
| Locked destination label/pixel/world coordinate | Independently compare activated route with mask alpha and POI label | `PENDING LIVE RUN` |
| `Ctrl+Alt+W` toggles in installed build | Observe over the real game window | `PENDING LIVE RUN` |
| Blue ray and repeated arrows visible | Capture actual game-window screenshot | `PENDING LIVE RUN` |
| U-turn/heading-unknown/waiting states truthful | Observe available real/simulated presentation states without game control | `PENDING LIVE RUN` |
| Alt-Tab hides the guide | Observe real game focus transition | `PENDING LIVE RUN` |
| Player reaches and drinks at destination | User-controlled walk/drink only | `PENDING USER CONTROLLED ACCEPTANCE` |

## Data-selection acceptance rule

The activated `targetMaskPx` must have alpha >= 128 in the same `freshwater.png`, lie on an inset freshwater boundary candidate, round-trip through `Calibration::islemaps()` to the route world coordinate, and use the nearest water POI only as its label. Failure of any check is `BLOCKED` and the guide must draw no ray.
