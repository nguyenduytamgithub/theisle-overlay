# Magnifier Night Boost v1.7.2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the ineffective flat tint with a verified Windows Magnification color transform that reveals real dark-scene detail through the existing Night Vision button.

**Architecture:** Keep the supervised `night-vision-filter` Tauri host but make its WebView transparent. A narrow audited Win32 adapter creates a click-through Magnifier child, maps the exact game client rectangle, applies a diagonal RGB gain matrix, excludes Overlay windows, and verifies readback before the controller reports ON.

**Tech Stack:** Rust 2021, Tauri 2, `windows` 0.62 Magnification API, TypeScript/CSS, Cargo tests, Svelte checks, NSIS.

## Global Constraints

- Version is `1.7.2`; build fingerprint is `1.7.2-magnifier-boost-a`.
- Strength 0 disables; 1–100 maps to `1.0 + 4.0 * strength / 100`, capped at 5.0.
- Do not open the game process, read memory, inject, hook DirectX, synthesize input, save/transmit frames, or add network access.
- Magnification screen-pixel use must be documented honestly and isolated to one audited adapter.
- Preserve the existing button, `Ctrl+Alt+N`, Windowed/Borderless requirement, focus lifecycle, and z-order.
- Public push/tag/release remains blocked until installed live-scene evidence and explicit user acceptance.

---

### Task 1: Contrast-preserving strength model

**Files:**
- Modify: `src-tauri/src/night_vision.rs`

**Interfaces:**
- Produces: `pub(crate) fn visual_boost_gain(strength: u8) -> f32`
- Removes: flat-tint-only `visual_boost_alpha`

- [ ] **Step 1: Write failing gain tests**

Add tests that require exact values `0 -> 1.0`, `1 -> 1.04`, `50 -> 3.0`, `70 -> 3.8`, `100/255 -> 5.0`, plus monotonicity for every adjacent strength.

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-nightboost'
cargo test --manifest-path src-tauri\Cargo.toml night_vision::tests::visual_boost_gain
```

Expected: FAIL because `visual_boost_gain` does not exist and current behavior is additive alpha.

- [ ] **Step 3: Implement the minimal gain function**

Use:

```rust
pub(crate) fn visual_boost_gain(strength: u8) -> f32 {
    1.0 + 4.0 * f32::from(strength.min(100)) / 100.0
}
```

- [ ] **Step 4: Run focused tests and verify GREEN**

Run the same command; require PASS.

- [ ] **Step 5: Commit**

```powershell
git add -- src-tauri/src/night_vision.rs
git commit -m "test: define contrast night boost curve"
```

---

### Task 2: Audited Windows Magnification adapter

**Files:**
- Create: `src-tauri/src/night_vision/magnifier.rs`
- Modify: `src-tauri/src/night_vision.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tests/night_vision_safety.rs`

**Interfaces:**
- Consumes: host HWND, `(left, top, width, height)`, gain, Overlay HWND list
- Produces:
  - `configure(host: isize, source: (i32, i32, i32, i32), gain: f32, excluded: &[isize]) -> Result<MagnifierReadback, MagnifierError>`
  - `destroy(host: isize) -> Result<(), MagnifierError>`
  - `is_configured(host: isize) -> bool`
  - `MagnifierReadback { source, gain, child: isize }`

- [ ] **Step 1: Write failing adapter/safety tests**

Add adapter unit tests for identity spatial matrix, diagonal RGB gain matrix,
source rectangle conversion, and strength bounds. Extend the safety test to
require exactly the reviewed imports:

```text
MagInitialize MagSetWindowSource MagGetWindowSource
MagSetWindowTransform MagSetColorEffect MagGetColorEffect
MagSetWindowFilterList WC_MAGNIFIER MW_FILTERMODE_EXCLUDE
CreateWindowExW DestroyWindow FindWindowExW IsWindow SetWindowPos
```

Continue rejecting process/memory/injection/hook/input/network/frame-save APIs.

- [ ] **Step 2: Run focused tests and verify RED**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-nightboost'
cargo test --manifest-path src-tauri\Cargo.toml --test night_vision_safety
cargo test --manifest-path src-tauri\Cargo.toml magnifier
```

Expected: FAIL because the adapter and `Win32_UI_Magnification` feature are absent.

- [ ] **Step 3: Implement the minimal adapter**

Enable `Win32_UI_Magnification`. Create/find one `WC_MAGNIFIER` child per host,
fill the host client area, apply the identity spatial transform and diagonal
gain matrix, set the exact desktop source, apply the exclusion list, and verify
source/color readback before returning success. Convert every failing BOOL into
an error containing the operation and `GetLastError` value. Destroy stale child
windows before host recreation.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run both focused commands; require PASS and no warnings.

- [ ] **Step 5: Commit**

```powershell
git add -- src-tauri/src/night_vision/magnifier.rs src-tauri/src/night_vision.rs src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tests/night_vision_safety.rs
git commit -m "feat: add audited magnifier night boost"
```

---

### Task 3: Replace painted tint acknowledgement with native readback

**Files:**
- Modify: `src-tauri/src/night_vision.rs`
- Modify: `src/night-vision-filter/main.ts`
- Modify: `src/night-vision-filter/style.css`

**Interfaces:**
- Consumes: existing filter host readiness/heartbeat, game rectangle, strength
- Produces: truthful `visualBoostApplied` only after native configure/readback and host visibility

- [ ] **Step 1: Write failing lifecycle tests**

Add tests that require:

- a matching pending request plus native readback and visible host marks applied;
- native failure never marks applied;
- OFF/Alt-Tab/game exit destroys the child and clears applied state;
- strength/geometry changes require a fresh configure/readback;
- stale request results are rejected; and
- normal operation does not open or apply a new gamma session.

- [ ] **Step 2: Run the lifecycle tests and verify RED**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-nightboost'
cargo test --manifest-path src-tauri\Cargo.toml night_vision::tests
```

Expected: FAIL because current success comes from a WebView paint event and normal reconcile still applies gamma.

- [ ] **Step 3: Implement native lifecycle integration**

Remove `FILTER_PAINT_EVENT`, `FILTER_PAINTED_EVENT`, `FILTER_COLOR`, and the
frontend paint request/ack path. Keep readiness/heartbeat. In the supervisor,
configure the magnifier after host geometry is synchronized; pass all Overlay
HWNDs to the exclusion list; accept the current request only after readback and
visibility; log request, strength, gain, child HWND, source rectangle, and
fingerprint. Destroy the child before hide, host close/recreate, OFF, Alt-Tab,
and exit. When the host is ready, reconcile must restore any legacy gamma
session rather than applying a new one.

- [ ] **Step 4: Make the host frontend truly transparent**

Keep only ready/heartbeat code. Set `html`, `body`, and the root surface to a
fully transparent background with no color/opacity layer and no pointer input.

- [ ] **Step 5: Run focused Rust and frontend gates**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-nightboost'
cargo test --manifest-path src-tauri\Cargo.toml night_vision::tests
npm run check
npm run build
```

Require PASS, 0 Svelte errors/warnings, and a production filter host asset.

- [ ] **Step 6: Commit**

```powershell
git add -- src-tauri/src/night_vision.rs src/night-vision-filter/main.ts src/night-vision-filter/style.css
git commit -m "fix: reveal real night scene detail"
```

---

### Task 4: Version, copy, safety documentation, and full automated gate

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src/main/settings/Settings.svelte`
- Modify: `src/lib/i18n/en.ts`
- Modify: `src/lib/i18n/vi.ts`
- Modify: `README.md`
- Modify: `README.en.md`
- Modify: `docs/verification/night-boost-v1.7.1.md`
- Create: `docs/verification/night-boost-v1.7.2.md`

**Interfaces:**
- Produces: v1.7.2 package identity and honest Magnification capture disclosure

- [ ] **Step 1: Write/update release configuration assertions**

Require version `1.7.2`, fingerprint suffix `magnifier-boost-a`, Magnification
capability text, and the absence of flat-tint copy.

- [ ] **Step 2: Run release/safety tests and verify RED**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-nightboost'
cargo test --manifest-path src-tauri\Cargo.toml --test release_config
cargo test --manifest-path src-tauri\Cargo.toml --test night_vision_safety
```

- [ ] **Step 3: Update package identity and user-facing copy**

Set all versions to 1.7.2. Explain that Night Vision locally redraws displayed
screen pixels with Windows Magnification, does not access game memory or inject,
requires Windowed/Borderless, may add composition cost, and is not claimed to be
server-approved. Mark v1.7.1 as rejected by live visual evidence.

- [ ] **Step 4: Run the complete automated gate**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-nightboost'
cargo fmt --manifest-path src-tauri\Cargo.toml -- --check
npm run check
npm run build
cargo test --manifest-path src-tauri\Cargo.toml --all-targets
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
npm run tauri build
```

Require every command PASS with no ignored failure.

- [ ] **Step 5: Commit**

```powershell
git add -- package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src/main/settings/Settings.svelte src/lib/i18n README.md README.en.md docs/verification
git commit -m "docs: qualify magnifier night boost"
```

---

### Task 5: Installed live-scene proof and release gate

**Files:**
- Modify: `docs/verification/night-boost-v1.7.2.md`
- Modify: release metadata only after explicit acceptance

**Interfaces:**
- Produces: installed v1.7.2 proof, OFF/ON captures and metrics; public release only after human PASS

- [ ] **Step 1: Record candidate identity**

Record commit, version, fingerprint, executable/installer paths, sizes, and SHA-256.

- [ ] **Step 2: Install without stopping The Isle**

Restore/destroy any active prototype, stop only TheIsle Overlay, install v1.7.2,
restart the Overlay, and verify the game PID remains alive.

- [ ] **Step 3: Capture and measure the same live night scene**

Capture OFF and ON at strength 70. In a HUD-free play-area sample require:

- ON mean luminance at least 3x OFF; and
- ON luminance standard deviation at least 1.5x OFF.

Open both captures visually and require foliage, terrain, and dinosaur detail to
be plainly distinguishable ON. Record the shown FPS and paired 10-second CPU,
working-set, and private-memory samples.

- [ ] **Step 4: Verify lifecycle and input**

Require click-through game input, filter hidden on Alt-Tab/OFF, fresh native
readback on return, no stale child HWND, no gamma recovery record, and clean exit.

- [ ] **Step 5: Obtain explicit user acceptance**

Leave v1.7.2 ON in the live scene and ask for exactly one result: accepted,
still too dark, or too bright. If not accepted, tune only the gain curve, repeat
tests/build/install/captures, and keep status PARTIAL.

- [ ] **Step 6: Publish only after PASS**

After explicit acceptance, rerun the full gate, update final verification,
push commits, create tag/release v1.7.2, link the original upstream, and mark
v1.7.0/v1.7.1 superseded without deleting history.
