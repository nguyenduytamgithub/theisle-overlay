# Visibility Engine v1.8.0 verification record

Date: 2026-08-29 (Asia/Bangkok)

Status: **PARTIAL — corrected candidate installed; automated gates and
same-image projection PASS, but fresh in-game night acceptance is still
required.**

## Scope and safety boundary

- Primary renderer: `gpu_adaptive` using Windows Graphics Capture on the exact
  The Isle HWND, D3D11, and a bounded local HLSL shader.
- Fallback renderer: `magnifier_fallback`, labelled separately and accepted
  only after native readback.
- No game-process handle, game-memory read/write, injection, hook, synthetic
  input, monitor/desktop capture, network upload, or frame file is used.
- Windowed or Borderless is required. The feature changes only local output;
  it does not change server day/night or weather and is independent of the
  game's X-key night vision.

## Source and implementation evidence

- Upstream source: <https://github.com/toantranct/theisle-overlay> (v1.5.2,
  `f628a18`).
- Community fork: <https://github.com/nguyenduytamgithub/theisle-overlay>.
- Adaptive tone model: `f3a04b9`.
- Truthful renderer state and luminance controller: `77968a1`.
- Magnifier fallback profiles: `8b2804c`.
- Exact-HWND GPU renderer and hardware probe: `1f24b55`.
- Preset/Force controls and fallback orchestration: `d2babe3`.
- Bright-scene washout protection and regression test: `8b8421f`.

## Fresh automated evidence

| Gate | Result |
|---|---|
| Svelte/TypeScript check | PASS — 0 errors, 0 warnings |
| Rust unit suite | PASS — 130 passed, 0 failed, 9 explicitly ignored live/cache tests |
| Night Vision safety suite | PASS — 6 passed, 0 failed |
| Release identity/copy suite | PASS — 4 passed, 0 failed |
| Clippy all targets/features | PASS with `-D warnings` |
| v1.8 feature-file rustfmt | PASS; repository-wide check still exposes unrelated legacy formatting divergence |
| Version consistency script | PASS — all release manifests report `1.8.0` |
| Forbidden-API script | PASS |
| Production frontend build | PASS |
| Tauri release + NSIS bundle | PASS |

The safety suite requires `CreateForWindow`, `CreateFreeThreaded`, D3D11,
flip-model presentation, and HLSL compilation while rejecting monitor capture,
desktop duplication, game-process/memory APIs, hooks, input synthesis,
networking, and file writes in the GPU adapter.

## Exact The Isle hardware smoke

The dev-only probe targeted live The Isle PID `35752`, HWND `133692`, client
rectangle `0,0,1920,1080`. Values are one local observation, not a fixed
performance promise.

| Readback | Observed |
|---|---:|
| Renderer | `gpu_adaptive` |
| Preset | `ultra` |
| Presented frames | 179 in 3 seconds |
| Median interval | 16.7043 ms |
| Presented FPS | 59.8648 |
| Scene luminance | 0.10605 |
| Final readback age | 86 ms |
| Probe exit | 0 |
| Game after teardown | still running |

No game toggle was sent and no frame was saved.

## Build, install, and live acceptance

Candidate artifacts built on this machine:

- Release executable:
  `D:\CodexBuild\theisle-overlay-visibility-v2\release\theisle-overlay.exe`
  — 21,587,968 bytes — SHA-256
  `4B9CAF4DC21091C8D53F158E4B6A9E00D801B944AFF5C0F4558E02FF81078ABF`.
- NSIS installer:
  `D:\CodexBuild\theisle-overlay-visibility-v2\release\bundle\nsis\TheIsle Overlay_1.8.0_x64-setup.exe`
  — 5,847,193 bytes — SHA-256
  `7F95FD2E678A2D7805ACE90A0EE31105FB3F1CACB211682E5B1016D7326794CD`.
- Both Windows version resources report `1.8.0`.

Fresh installation evidence for the corrected candidate:

- Installed executable:
  `C:\Users\Admin\AppData\Local\TheIsle Overlay\theisle-overlay.exe`
  — 21,587,968 bytes — SHA-256
  `DB0CAAB8BC2EA1AD101A2FE19C3AC7BFC2DB11A4A38A4F6918FFB52F82C15CD6`
  — Windows file/product version `1.8.0`.
- NSIS silent install exit: `0`. The installed PE hash is recorded separately
  from the raw release executable because Tauri patches NSIS bundle metadata.
- Overlay PID `40100` was alive and responding after installation. Game PID
  `35048` and client PID `19724` stayed alive and responding across the
  Overlay-only stop/install/start. No game input was sent.
- The previous installed candidate was 21,597,696 bytes, SHA-256
  `F13894CACF9B2350792074B3949AB31BDEFDD26F2945AF27026CB93BA36AD779`.
- Pre-install v1.7.4 executable: 21,459,968 bytes, SHA-256
  `301369E5245AC7959B4BAFCFB0CDEAB24964494FC0098475AE797556324EC793`.
- Preserved rollback installer: 5,814,158 bytes, SHA-256
  `9F341680E91BF3D20F74DCD9B3F06271D9C6CD63C85DEBB2EB2AE247E714699C`.
- Immediately after installation, `waypoints.json` still had its pre-install
  452-byte SHA-256
  `95050455912E54612259B878CD92089254029E579C21247043B2BA0C2EA845A8`.
  At `17:48:35`, while the game and Overlay remained running, the file gained
  one waypoint and became 657 bytes with SHA-256
  `F14B209208BA9B458EA9A2A2CD202B55B06B919EE3C415B04041F01BC3ADB716`.
  Both prior waypoint IDs remain; one prior Y coordinate differs only by
  `2.91e-11` serialization noise. No waypoint was restored or overwritten.
  `settings.json` retained the requested Ultra/Force/GPU-preferred strength-85
  configuration. Existing trail files were retained; the active session trail
  remained append-owned by the running app before the upgrade.

## Washout reproduction and correction

Local evidence is under
`D:\CodexBuild\theisle-overlay-visibility-v2\live-proof`.

The first installed v1.8 candidate was rejected after a real game observation:

| Sample | Median | P05 | P95 | Local contrast | Edge energy | Clip ratio |
|---|---:|---:|---:|---:|---:|---:|
| `01-off.png` | 0.00280 | 0.00000 | 0.18413 | 0.01018 | 0.00364 | 0.00000 |
| `02-x-only.png` | 0.28912 | 0.05355 | 0.51152 | 0.04502 | 0.01739 | 0.00000 |
| `03-x-plus-ultra.png` (old candidate) | 0.66863 | 0.38494 | 0.87400 | 0.08159 | 0.05582 | 0.00037 |

`03-x-plus-ultra.png` is a **FAIL** despite low literal clipping: its P05 and
median show an unusably elevated shadow floor/grey wash. Samples 2 and 3 also
had a camera change, so they are evidence of the reported defect but are not a
valid final paired acceptance.

Commit `8b8421f` now fades global exposure/lift/gamma changes as measured scene
luminance rises while retaining bounded local-detail gain. On the exact
`02-x-only.png` source, the deterministic projected v2 output
`06-x-plus-ultra-v2-projected.png` measured:

- median `0.35322`, P05 `0.09058`, P95 `0.57249`;
- local contrast `0.05568` (**+23.7%** versus X-only);
- edge energy `0.02936` (**+68.8%** versus X-only); and
- clip ratio `0.00000`.

This same-image projection is a parameter/model gate, **not installed-runtime
proof**. The corrected installed candidate still needs a fresh unchanged-camera
X-only versus X+Ultra nighttime comparison and explicit user acceptance.

Remaining before public release:

- Corrected candidate same-scene X-only / X+Ultra live images and metrics:
  **PENDING**.
- User confirms X+Ultra is clearer without grey wash or lost terrain detail:
  **PENDING**.

Do not call v1.8.0 complete or publish/push it until every pending item above
has evidence.
