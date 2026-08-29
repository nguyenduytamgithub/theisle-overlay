# Visibility Engine v1.8.0 verification record

Date: 2026-08-29 (Asia/Bangkok)

Status: **PARTIAL — automated and exact-HWND GPU smoke PASS; installed
same-scene dark acceptance is still required.**

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

## Fresh automated evidence

| Gate | Result |
|---|---|
| Svelte/TypeScript check | PASS — 0 errors, 0 warnings |
| Rust unit suite | PASS — 129 passed, 0 failed, 9 explicitly ignored live/cache tests |
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
  — 21,597,696 bytes — SHA-256
  `07B3467942C688E0C8D8A1536180DED0867DE030C4F89D22D9E6BC7937786DBE`.
- NSIS installer:
  `D:\CodexBuild\theisle-overlay-visibility-v2\release\bundle\nsis\TheIsle Overlay_1.8.0_x64-setup.exe`
  — 5,847,778 bytes — SHA-256
  `444DC0AE22E769B539B30FD041CB4051AE578EA34AA516FF1E5601EF66E1C8A3`.
- Both Windows version resources report `1.8.0`.

These installed/live fields must still be filled from fresh evidence before
public release:

- Installed executable path/bytes/SHA-256: **PENDING**
- Installed fingerprint `1.8.0-gpu-visibility-c`: **PENDING**
- Installed runtime renderer/FPS/luma readback: **PENDING**
- Same-scene OFF / game-X-only / X+Ultra images and metrics: **PENDING**
- User confirms X+Ultra is materially clearer without unusable highlight
  clipping: **PENDING**

Do not call v1.8.0 complete or publish/push it until every pending item above
has evidence.
