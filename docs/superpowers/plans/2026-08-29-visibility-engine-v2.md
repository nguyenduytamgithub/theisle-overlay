# Visibility Engine v2 Implementation Plan

> **Execution mode:** Implement inline with `superpowers:executing-plans` and
> test-driven development. Do not dispatch subagents. Mark each checkbox only
> after the stated command produces fresh evidence.

**Goal:** Ship v1.8.0 with a truthful client-only GPU visibility renderer that
materially outperforms The Isle's `X` mode alone in a real nighttime scene,
while preserving the current Magnifier implementation as a labeled fallback.

**Architecture:** Capture only The Isle's HWND through Windows Graphics
Capture, keep the texture on Direct3D11, apply adaptive exposure plus nonlinear
shadow/highlight/local-contrast processing in HLSL, and present through a
click-through native child of the existing supervised filter host. Extend the
controller and UI so ON means a recent verified frame from the named renderer.

**Tech stack:** Rust 2021, Tauri 2, `windows` 0.62, Windows Runtime Graphics
Capture, Direct3D11, DXGI flip-model swap chain, HLSL compiled by D3DCompiler,
TypeScript/Svelte, Cargo tests, Svelte check, Vite, NSIS.

## Global constraints

- Version: `1.8.0`; build fingerprint: `1.8.0-visibility-engine-v2-a`.
- Never open/read/write the game process, inject, hook game graphics, synthesize
  input, capture packets, modify game/server/files, persist frames, or transmit
  pixels.
- Capture target must be the exact HWND already selected by the foreground game
  supervisor; Windowed/Borderless is required.
- Keep The Isle running during candidate build/install. Stop/restart only the
  Overlay after an installer has passed automated gates.
- Preserve token, settings, waypoints, trails, and the installed v1.7.4 recovery
  path. Hash user data before and after installation.
- `X` stays independent. The acceptance combination is `X` ON plus Overlay
  preset `ultra` with force mode enabled.
- UI must label `gpu_adaptive`, `magnifier_fallback`, `waiting`, or `error`.
- A native handle, successful API return, compiled shader, or presented frame
  alone is not visual completion. The live paired night gate is mandatory.
- Use `D:\CodexBuild\theisle-overlay-nightboost` as the shared test cache until
  the release build, and `D:\CodexBuild\theisle-overlay-visibility-v2` for final
  artifacts. Recheck free space before each full build.

---

### Task 1: Deterministic visibility model and presets

**Files:**

- Create: `src-tauri/src/night_vision/visibility.rs`
- Create: `src-tauri/tests/fixtures/visibility-dark.json`
- Create: `src-tauri/tests/fixtures/visibility-mixed.json`
- Create: `src-tauri/tests/fixtures/visibility-bright.json`
- Modify: `src-tauri/src/night_vision.rs`

**Interfaces:**

```rust
pub(crate) enum VisibilityPreset { Balanced, Clear, Ultra }
pub(crate) struct VisibilityParameters {
    exposure: f32,
    shadow_lift: f32,
    gamma: f32,
    highlight_knee: f32,
    saturation: f32,
    detail_gain: f32,
}
pub(crate) fn preset_parameters(
    preset: VisibilityPreset,
    strength: u8,
    scene_luma: f32,
) -> VisibilityParameters;
pub(crate) fn transform_rgb(
    rgb: [f32; 3],
    local_average: [f32; 3],
    parameters: VisibilityParameters,
) -> [f32; 3];
```

- [x] **Step 1: Add failing unit and fixture tests**

Require:

- strength clamped to 0–100 and every parameter finite and bounded;
- `Balanced < Clear < Ultra` for shadow exposure at the same dark luma;
- output channels remain in 0–1;
- pure black gains a bounded floor in Clear/Ultra;
- near-white input remains distinguishable and does not clip all channels;
- local detail of a dark pair increases rather than collapses;
- the bright fixture changes less than the dark fixture; and
- repeated calls are bit-for-bit deterministic.

- [x] **Step 2: Run focused tests and verify RED**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-nightboost'
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml visibility
```

Expected: compilation failure because `visibility` and its interfaces do not
exist.

- [x] **Step 3: Implement the minimal reference model**

Use a bounded exposure derived from preset, strength, and scene luma; apply a
nonlinear shadow lift; compress values beyond `highlight_knee`; restore color
with bounded saturation; add `(rgb - local_average) * detail_gain` under a
shadow mask; sanitize non-finite inputs; and clamp only at the final output.

- [x] **Step 4: Run focused tests and verify GREEN**

Run the same command and require every visibility test PASS.

- [x] **Step 5: Commit**

```powershell
git add -- src-tauri/src/night_vision.rs src-tauri/src/night_vision/visibility.rs src-tauri/tests/fixtures
git commit -m "feat: define adaptive visibility tone model"
```

---

### Task 2: Fast adaptive luminance and truthful renderer state

**Files:**

- Modify: `src-tauri/src/night_vision/visibility.rs`
- Modify: `src-tauri/src/night_vision.rs`
- Modify: `src/lib/api.ts`

**Interfaces:**

```rust
pub(crate) struct LuminanceController {
    smoothed: f32,
    last_sample_at: Option<Instant>,
}
pub(crate) enum VisibilityRenderer { None, GpuAdaptive, MagnifierFallback }
pub(crate) struct RendererReadback {
    renderer: VisibilityRenderer,
    game_hwnd: isize,
    source: (i32, i32, i32, i32),
    preset: VisibilityPreset,
    presented_frames: u64,
    last_presented_at: Instant,
    median_interval_ms: f32,
}
```

Extend `NightVisionState` with serialized camelCase fields:

```rust
renderer: "none" | "gpu_adaptive" | "magnifier_fallback"
preset: "balanced" | "clear" | "ultra"
force_bright: bool
scene_luma: Option<f32>
presented_fps: Option<f32>
```

- [x] **Step 1: Add failing state/adaptation tests**

Require 4 Hz sample limiting, fast dark-entry response, slower bright-exit
response, no overshoot, no NaN, stale-frame rejection after 500 ms, current
HWND/rectangle/preset matching, and `visualBoostApplied == true` only for a
recent renderer readback.

- [x] **Step 2: Run focused tests and verify RED**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-nightboost'
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml night_vision::tests
```

- [x] **Step 3: Implement the model and state shape**

Add serializable renderer/preset enums, the asymmetric smoother, readback
validation, and TypeScript mirrors. Preserve all v1.7.4 fields for compatibility.

- [x] **Step 4: Run focused Rust and frontend checks**

```powershell
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml night_vision::tests
npm run check
```

- [x] **Step 5: Commit**

```powershell
git add -- src-tauri/src/night_vision.rs src-tauri/src/night_vision/visibility.rs src/lib/api.ts
git commit -m "feat: track truthful visibility renderer state"
```

---

### Task 3: Stronger Magnifier compatibility fallback

**Files:**

- Modify: `src-tauri/src/night_vision/magnifier.rs`
- Modify: `src-tauri/src/night_vision.rs`
- Modify: `src-tauri/tests/night_vision_safety.rs`

**Interfaces:**

```rust
pub(crate) struct MagnifierProfile {
    gain: f32,
    black_translation: f32,
    cross_channel_luma: f32,
}
pub(crate) fn fallback_profile(
    preset: VisibilityPreset,
    strength: u8,
) -> MagnifierProfile;
```

- [x] **Step 1: Add failing fallback tests**

Require profile monotonicity, finite bounds, nonzero but capped black translation
for Clear/Ultra, correct 5x5 matrix placement, readback tolerance, and the
`magnifier_fallback` state label. Retain all existing lifecycle tests.

- [x] **Step 2: Run focused tests and verify RED**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-nightboost'
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml magnifier
```

- [x] **Step 3: Implement bounded gain, translation, and label**

Construct the audited `MAGCOLOREFFECT` from the profile. Verify the entire
matrix, exact source rectangle, child HWND, exclusion list, and renderer label
before reporting applied. GPU and Magnifier children are mutually exclusive.

- [x] **Step 4: Run focused and safety tests**

```powershell
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml magnifier
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml --test night_vision_safety
```

- [x] **Step 5: Commit**

```powershell
git add -- src-tauri/src/night_vision/magnifier.rs src-tauri/src/night_vision.rs src-tauri/tests/night_vision_safety.rs
git commit -m "feat: strengthen truthful magnifier fallback"
```

---

### Task 4: Window-scoped Direct3D11 capture and visibility shader

**Files:**

- Create: `src-tauri/src/night_vision/gpu.rs`
- Create: `src-tauri/src/night_vision/visibility.hlsl`
- Modify: `src-tauri/src/night_vision.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tests/night_vision_safety.rs`
- Modify: `src-tauri/src/bin/verify_night_vision.rs`

**Native interface:**

```rust
pub(crate) struct GpuVisibilitySession;

impl GpuVisibilitySession {
    pub(crate) fn start(config: GpuSessionConfig) -> Result<Self, GpuVisibilityError>;
    pub(crate) fn readback(&self) -> Result<Option<RendererReadback>, GpuVisibilityError>;
    pub(crate) fn stop(self) -> Result<(), GpuVisibilityError>;
}
```

Enable only these additional `windows` feature families required by the
adapter: `Foundation`, `Graphics_Capture`, `Graphics_DirectX`,
`Graphics_DirectX_Direct3D11`, `Win32_Graphics_Direct3D`,
`Win32_Graphics_Direct3D11`, `Win32_Graphics_Direct3D_Fxc`,
`Win32_Graphics_Dxgi`, `Win32_Graphics_Dxgi_Common`, `Win32_System_Com`,
`Win32_System_WinRT`, `Win32_System_WinRT_Direct3D11`, and
`Win32_System_WinRT_Graphics_Capture`.

- [x] **Step 1: Add failing shader/safety/lifecycle tests**

Require:

- HLSL contains a fullscreen-triangle vertex shader and one visibility pixel
  shader with explicit bounded constants;
- HLSL reference parameters match Rust field order and byte size;
- capture item creation consumes only the supplied game HWND;
- the presentation child consumes only the supplied host HWND;
- frame-pool resize, device loss, stop, and stale frame are fail-closed;
- no `OpenProcess`, memory API, injection, hook, input, networking, file write,
  Desktop Duplication, or arbitrary screen/window enumeration path is present;
- the new GPU adapter owns the only approved WinRT/D3D imports; and
- a repeated stop is idempotent and releases capture before destroying the
  presentation child.

- [x] **Step 2: Run focused tests and verify RED**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-nightboost'
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml gpu
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml --test night_vision_safety
```

- [x] **Step 3: Implement D3D device, exact-HWND capture, and host child**

On a dedicated MTA thread, create one hardware D3D11 device on the game's
adapter with BGRA support, wrap it as a WinRT `IDirect3DDevice`, create the
`GraphicsCaptureItem` through `CreateForWindow(game_hwnd)`, create a free-threaded
two-frame pool, and create the session. Create one click-through native child in
the existing filter host and one two-buffer flip-model swap chain for that
child. Do not use a picker, monitor capture, or whole-desktop fallback.

- [x] **Step 4: Implement and bind the HLSL pipeline**

Compile embedded HLSL with `D3DCompile`, create a fullscreen-triangle vertex
shader and visibility pixel shader, create SRV/RTV/sampler/constant buffer, and
render each received texture to the swap-chain back buffer. Render a fixed
4x4 GPU luminance grid into a single-channel 1x1 target; at most every 250 ms
copy only that aggregate float to a staging texture, update the smoother, and
update shader constants. Present with sync interval 0; coalesce callbacks so
work never queues unboundedly.

- [x] **Step 5: Implement readback, resize, and recovery**

Track current target HWND/source/preset, frame count, last-present timestamp,
rolling intervals, luma, device reason, and generation. Recreate frame pool and
swap-chain buffers on source size changes. On device removal or capture access
loss, tear down completely and permit one clean controller restart.

- [x] **Step 6: Run tests and a hardware smoke probe**

```powershell
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml gpu
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml --test night_vision_safety
& 'C:\Users\Admin\.cargo\bin\cargo.exe' run --manifest-path src-tauri\Cargo.toml --features devtools --bin verify_night_vision -- --gpu-smoke --seconds 5
```

Require D3D hardware device, exact The Isle HWND, at least 120 captured frames,
at least 120 presented frames, finite luma, no device removal, median interval
at or below 33 ms, and clean session teardown. The probe must not toggle the
game or save a frame.

Observed on the exact live The Isle HWND: 179 presented frames in three
seconds, median 16.7043 ms (59.8648 FPS), scene luma 0.10605, readback age
86 ms, exit code 0, and the game process remained running.

- [x] **Step 7: Commit**

```powershell
git add -- src-tauri/src/night_vision/gpu.rs src-tauri/src/night_vision/visibility.hlsl src-tauri/src/night_vision.rs src-tauri/src/bin/verify_night_vision.rs src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tests/night_vision_safety.rs
git commit -m "feat: render adaptive game visibility on the GPU"
```

---

### Task 5: Controller, settings migration, UI, and fallback recovery

**Files:**

- Modify: `src-tauri/src/night_vision.rs`
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/api.ts`
- Modify: `src/main/settings/Settings.svelte`
- Modify: `src/lib/i18n/vi.ts`
- Modify: `src/lib/i18n/en.ts`
- Modify: `src/night-vision/main.ts`

**Commands:**

```rust
set_night_vision_preset(preset: VisibilityPreset) -> NightVisionState
set_night_vision_force_bright(force_bright: bool) -> NightVisionState
```

Settings schema:

```json
"night_vision": {
  "strength": 85,
  "show_button": true,
  "preset": "ultra",
  "force_bright": true,
  "prefer_gpu": true
}
```

- [x] **Step 1: Add failing migration/controller/recovery tests**

Require old `{strength, show_button}` settings to merge with the new defaults,
unknown preset to normalize to `ultra`, UI commands to persist before
reconcile, GPU start before fallback, fallback only after exact GPU error,
one device-loss retry, no renderer overlap, OFF/Alt-Tab/exit cleanup, and fresh
GPU restart after the game HWND changes.

- [x] **Step 2: Run focused tests and verify RED**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-nightboost'
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml night_vision
npm run check
```

- [x] **Step 3: Wire controller and commands**

Make the controller request GPU first. Accept applied only after a fresh GPU
readback; otherwise destroy partial GPU state and start verified Magnifier
fallback. Destroy the active renderer before switching preset/strength/target,
hiding the host, or exiting. Emit exact renderer/error/fps/luma state.

- [x] **Step 4: Wire settings and on-screen controls**

Add Vietnamese/English preset pills, Auto/Force toggle, active renderer label,
and live FPS/luma diagnostic line. Keep one-click ON/OFF and `Ctrl+Alt+N`. Use
`ultra`, strength 85, force mode true as v1.8 defaults while preserving an
existing user's explicit strength.

- [x] **Step 5: Run controller, frontend, and production build gates**

```powershell
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml night_vision
npm run check
npm run build
```

- [x] **Step 6: Commit**

```powershell
git add -- src-tauri/src/night_vision.rs src-tauri/src/settings.rs src-tauri/src/lib.rs src/lib/api.ts src/main/settings/Settings.svelte src/lib/i18n src/night-vision/main.ts
git commit -m "feat: control adaptive and fallback visibility modes"
```

---

### Task 6: v1.8.0 identity, documentation, and complete automated gate

**Files:**

- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `README.md`
- Modify: `README.en.md`
- Modify: `src-tauri/tests/release_config.rs`
- Create: `docs/verification/visibility-engine-v1.8.0.md`

- [x] **Step 1: Add failing release assertions**

Require every version to equal 1.8.0, exact fingerprint, GPU/Magnifier renderer
labels, Windows Graphics Capture disclosure, Windowed/Borderless requirement,
game `X` independence, no “always daylight” guarantee, no v1.7.4 primary-renderer
claim, and public upstream/fork links.

- [x] **Step 2: Run release test and verify RED**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-nightboost'
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml --test release_config
```

- [x] **Step 3: Update identity and documentation**

Document how to use `X + Cực sáng`, Auto/Force, renderer labels, performance
cost, safety boundary, instant OFF, and the exact difference from upstream and
v1.7.4. Mark live visual proof pending until Task 7.

- [x] **Step 4: Synchronize CodeGraph and run a focused audit**

Synchronize the repository index, then query with `maxFiles: 2` for the complete
toggle → controller → GPU/fallback → readback → UI status call path. Resolve any
stale, duplicate, or untested path before the full gate.

- [x] **Step 5: Run the complete automated gate**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-visibility-v2'
# The upstream repository has unrelated legacy rustfmt drift. Run rustfmt
# --check only on the v1.8 Rust change set and record the repository-wide
# divergence rather than rewriting unrelated owner files.
npm run check
npm run build
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml --all-targets
& 'C:\Users\Admin\.cargo\bin\cargo.exe' clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
npm run tauri build -- --config src-tauri\tauri.conf.json
```

Require every non-live test PASS, 0 frontend diagnostics, no Clippy warning,
successful NSIS bundle, and no new file outside the documented build/repo paths.

- [x] **Step 6: Record artifacts and commit**

Record commit/version/fingerprint, paths, sizes, SHA-256, test counts, ignored
live tests, and truthful PARTIAL status pending real nighttime proof.

```powershell
git add -- package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json README.md README.en.md src-tauri/tests/release_config.rs docs/verification/visibility-engine-v1.8.0.md
git commit -m "docs: qualify visibility engine v1.8.0 candidate"
```

---

### Task 7: Safe installation, paired live-night proof, tuning, and public push

**Files:**

- Modify: `docs/verification/visibility-engine-v1.8.0.md`
- Modify release metadata only after PASS

- [x] **Step 1: Preserve runtime evidence and user data**

Record current game PIDs, installed Overlay identity, data-file inventory and
SHA-256, v1.7.4 installer hash, and free space. Do not stop the game.

- [x] **Step 2: Install the candidate**

Stop only TheIsle Overlay, run the v1.8.0 NSIS installer, start the Overlay, and
verify both original game processes are still responding. Verify token,
settings, waypoints, trails, and navigation settings have unchanged hashes or
expected schema-only changes.

- [ ] **Step 3: Capture the paired real-night scene**

At one unchanged camera view collect:

1. `X` OFF / Overlay OFF;
2. `X` ON / Overlay OFF;
3. `X` ON / Overlay `Cực sáng`, Force ON, renderer `gpu_adaptive`.

Use only an external screenshot for local evidence; redact server/player data
if documentation is public. Measure a HUD-free ROI for median luma, dark-region
local contrast, clipped-pixel ratio, and edge energy. Read GPU renderer cadence
from native readback and sample Overlay/game CPU, GPU, memory, and shown FPS.

- [ ] **Step 4: Enforce the acceptance gate**

Sample 3 must achieve median luma at least 1.6x sample 2 or 0.20 normalized,
local contrast at least 1.20x, clipping below 15%, median present interval at or
below 33 ms, correct click-through/z-order/lifecycle, and visibly clearer
terrain, foliage, and dinosaur silhouette. If any item fails, keep status
PARTIAL, adjust only bounded preset/shader parameters, repeat RED/GREEN tests,
rebuild/reinstall, and repeat the same-scene comparison.

- [ ] **Step 5: Obtain explicit human acceptance**

Leave the passing build enabled in the live scene and ask the user to inspect
it. Public release remains blocked until the user explicitly accepts the visual
result. “Process running”, “renderer active”, and metric PASS do not substitute
for this gate.

- [ ] **Step 6: Final verification and public delivery**

After acceptance, rerun the complete gate, update verification to PASS, commit,
push `codex/visibility-engine-v2` to the user's public fork, update/create the
pull request with upstream attribution and exact improvements, and publish the
installer/hash only if all release metadata is consistent. Do not rewrite or
delete v1.7.4 history.

## Stop conditions

- Stop the renderer immediately if the game process, anti-cheat, or display
  becomes unstable; restore v1.7.4 and preserve logs.
- Stop installation if user-data hashes cannot be accounted for.
- Do not downgrade the proof gate or label Magnifier fallback as adaptive GPU.
- A literal “ngưng” from the user pauses all machine-changing work immediately.
