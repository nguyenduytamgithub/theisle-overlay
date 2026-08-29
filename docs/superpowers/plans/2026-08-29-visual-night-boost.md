# Visual Night Boost v1.7.1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the visually ineffective gamma-only result with a deterministic, click-through brightness veil while retaining gamma as an independently reported supplement.

**Architecture:** A hidden `night-vision-filter` Tauri WebView covers the exact game client rectangle and paints a static green-white alpha layer. Rust owns request IDs, window visibility, bounds, z-order, and truthful state; the WebView returns a painted acknowledgement after two animation frames. The existing gamma controller remains, but `ON` is derived only from the acknowledged and visible filter.

**Tech Stack:** Rust, Tauri 2, Windows user32 window styles, TypeScript, Vite, CSS, Svelte settings UI.

## Global Constraints

- Never read or write game memory, inject code, hook DirectX, capture pixels, synthesize input, modify game files, or use the network.
- Strength 0 maps to alpha 0; strength 1 through 100 maps to `0.05 + 0.0025 * strength`.
- The filter color is `rgb(235, 240, 230)` and default strength is 70.
- `NHÌN ĐÊM: BẬT` is permitted only after the latest painted acknowledgement and Windows visibility readback.
- Alt+Tab, main-window foreground, game exit, overlay exit, and updater relaunch hide the filter and restore gamma.
- The local candidate is version 1.7.1 and must expose a build fingerprint.
- No public release or completion claim is allowed before the user explicitly confirms `thấy sáng rõ hơn` in a real night scene.

---

### Task 1: Truthful visual state and opacity model

**Files:**
- Modify: `src-tauri/src/night_vision.rs`

**Interfaces:**
- Produces: `visual_boost_alpha(strength: u8) -> f64`
- Produces: `NightVisionState.visual_boost_ready`, `visual_boost_applied`, `gamma_applied`, and `build_fingerprint`
- Produces: controller methods `mark_filter_ready`, `begin_visual_request`, `accept_visual_paint`, `clear_visual_applied`, and `mark_filter_failed`

- [ ] **Step 1: Write failing unit tests**

Add literal, independently derived assertions:

```rust
#[test]
fn visual_boost_alpha_has_the_approved_bounds_and_default() {
    assert_eq!(super::visual_boost_alpha(0), 0.0);
    assert!((super::visual_boost_alpha(1) - 0.0525).abs() < f64::EPSILON);
    assert!((super::visual_boost_alpha(70) - 0.225).abs() < f64::EPSILON);
    assert!((super::visual_boost_alpha(100) - 0.30).abs() < f64::EPSILON);
    assert!((super::visual_boost_alpha(u8::MAX) - 0.30).abs() < f64::EPSILON);
}

#[test]
fn stale_paint_ack_never_turns_visual_boost_on() {
    let (controller, _) = controller(false);
    controller.toggle_requested();
    controller.mark_filter_ready();
    let request_id = controller.begin_visual_request().unwrap();
    assert!(!controller.accept_visual_paint(request_id - 1, 70, true).applied);
    assert!(controller.accept_visual_paint(request_id, 70, true).applied);
}

#[test]
fn gamma_failure_does_not_disable_ready_visual_fallback() {
    let (controller, _) = controller(true);
    controller.toggle_requested();
    controller.mark_filter_ready();
    controller.reconcile(Some(target("DISPLAY1", 101)));
    let request_id = controller.begin_visual_request().unwrap();
    let state = controller.accept_visual_paint(request_id, 70, true);
    assert!(state.applied && state.visual_boost_applied);
    assert!(!state.gamma_applied);
    assert!(state.supported);
}
```

- [ ] **Step 2: Run the focused Rust tests and verify RED**

Run:

```powershell
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml --lib night_vision::tests::visual_boost -- --nocapture
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml --lib night_vision::tests::stale_paint_ack -- --nocapture
```

Expected: compilation fails because the visual fields, alpha function, and acknowledgement methods do not exist.

- [ ] **Step 3: Implement the minimal state model**

Use this state contract:

```rust
pub struct NightVisionState {
    pub requested: bool,
    pub applied: bool,
    pub supported: bool,
    pub strength: u8,
    pub error_key: Option<String>,
    pub visual_boost_ready: bool,
    pub visual_boost_applied: bool,
    pub gamma_applied: bool,
    pub build_fingerprint: &'static str,
}

fn visual_boost_alpha(strength: u8) -> f64 {
    let strength = strength.min(100);
    if strength == 0 { 0.0 } else { 0.05 + 0.0025 * f64::from(strength) }
}
```

Store `pending_visual_request: Option<(u64, u8)>` and a monotonically increasing request ID in `ControllerInner`. `accept_visual_paint` must require the pending tuple, matching strength, current `requested = true`, and `window_visible = true`. Set legacy `applied` equal to `visual_boost_applied`; keep gamma outcome in `gamma_applied`. A gamma apply error sets the gamma warning key but leaves `supported = true` when the filter is ready.

- [ ] **Step 4: Run all night-vision unit tests and verify GREEN**

Run:

```powershell
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml --lib night_vision::tests -- --nocapture
```

Expected: all night-vision tests pass, including updated legacy assertions that now check `gamma_applied` instead of treating gamma as visual proof.

- [ ] **Step 5: Commit Task 1**

```powershell
git add -- src-tauri/src/night_vision.rs
git commit -m "feat: model truthful visual night boost state"
```

### Task 2: Static filter WebView and painted acknowledgement

**Files:**
- Create: `night-vision-filter.html`
- Create: `src/night-vision-filter/main.ts`
- Create: `src/night-vision-filter/style.css`
- Modify: `vite.config.ts`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `src-tauri/src/webview_mem.rs`
- Modify: `src-tauri/tests/night_vision_safety.rs`

**Interfaces:**
- Consumes event: `night-vision-filter://paint` with `{ requestId: number, strength: number, alpha: number, color: string }`
- Produces event: `night-vision-filter://ready`
- Produces event: `night-vision-filter://painted` with `{ requestId: number, strength: number }`
- Produces event: `night-vision-filter://heartbeat`

- [ ] **Step 1: Extend the safety test first**

Add `night-vision-filter` assets to the source scan and assert the filter entry contains no capture, canvas loop, network, input-synthesis, or game-process APIs. Extend the allowed-window manifests to require the `night-vision-filter` label.

- [ ] **Step 2: Run the safety test and verify RED**

Run:

```powershell
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml --test night_vision_safety -- --nocapture
```

Expected: failure because the filter files and window capability are absent.

- [ ] **Step 3: Create the static filter entry**

The HTML body contains only `<div id="veil" aria-hidden="true"></div>`. TypeScript listens for paint requests, validates `requestId`, clamps strength and alpha, sets `backgroundColor` and `opacity`, waits for two `requestAnimationFrame` callbacks, then emits the matching painted event. It emits ready once and heartbeat every two seconds. CSS fills the viewport, stays transparent outside the veil, disables selection, and uses `pointer-events: none` on every element.

- [ ] **Step 4: Register build, capability, and memory-watchdog entries**

Add `nightVisionFilter` to Vite inputs, `night-vision-filter` to the Tauri capability window list, and `night-vision-filter` to the WebView memory watchdog's label array.

- [ ] **Step 5: Verify frontend and safety GREEN**

Run:

```powershell
npm run check
npm run build
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml --test night_vision_safety -- --nocapture
```

Expected: Svelte check has zero errors/warnings, Vite emits the filter entry, and the safety suite passes.

- [ ] **Step 6: Commit Task 2**

```powershell
git add -- night-vision-filter.html src/night-vision-filter vite.config.ts src-tauri/capabilities/default.json src-tauri/src/webview_mem.rs src-tauri/tests/night_vision_safety.rs
git commit -m "feat: add click-through night boost filter"
```

### Task 3: Filter window lifecycle, bounds, z-order, and acknowledgement gate

**Files:**
- Modify: `src-tauri/src/night_vision.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `create_filter(app: &AppHandle) -> tauri::Result<()>`
- Produces serializable `FilterPaintRequest { request_id, strength, alpha, color }`
- Consumes `night-vision-filter://ready`, `night-vision-filter://painted`, and heartbeat events

- [ ] **Step 1: Write failing lifecycle tests**

Add pure tests for these observable decisions:

```rust
#[test]
fn filter_visibility_requires_request_ready_and_foreground_game() {
    assert!(super::filter_should_show(true, true, true, false));
    assert!(!super::filter_should_show(false, true, true, false));
    assert!(!super::filter_should_show(true, false, true, false));
    assert!(!super::filter_should_show(true, true, false, false));
    assert!(!super::filter_should_show(true, true, true, true));
}

#[test]
fn hidden_filter_clears_visual_applied_but_preserves_request() {
    let (controller, _) = controller(false);
    controller.toggle_requested();
    controller.mark_filter_ready();
    let request_id = controller.begin_visual_request().unwrap();
    controller.accept_visual_paint(request_id, 70, true);
    let state = controller.clear_visual_applied("night_vision.waiting_for_game");
    assert!(state.requested);
    assert!(!state.applied && !state.visual_boost_applied);
}
```

- [ ] **Step 2: Run tests and verify RED**

Run the two named tests and require failures caused by missing filter lifecycle code.

- [ ] **Step 3: Build the hidden owned window**

Create `night-vision-filter` before the button with transparent, undecorated, no-shadow, topmost, skipped-taskbar, non-resizable, non-focusable, hidden properties. Register its HWND, call `assert_overlay_styles`, enable click-through through both Tauri and `win::overlay::set_click_through`, and never disable click-through.

- [ ] **Step 4: Implement the supervisor and state gate**

At 250 ms ticks, locate the game HWND, debounce focus by two ticks, hide the filter when the game is absent/iconic/unfocused or `main` is foreground, and restore gamma through the existing reconcile path. When effective, synchronize the exact physical client rectangle, emit the latest paint request, show, wait for visibility, then accept only the matching painted acknowledgement. On each show/z-order repair, raise windows in this order: filter, minimap, HUD, Night Vision button.

Recreate a missing or unhealthy filter once per failure incident after five seconds. A second failed paint leaves `visual_boost_applied = false` and an error key until the next user toggle or app restart.

- [ ] **Step 5: Wire startup and exit**

Call `night_vision::create_filter(app.handle())?` before `create_button`. Ensure `restore_before_exit` hides the filter before returning and treats any visible filter as an incomplete exit cleanup.

- [ ] **Step 6: Verify lifecycle GREEN**

Run all library and safety tests. Then run `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.

- [ ] **Step 7: Commit Task 3**

```powershell
git add -- src-tauri/src/night_vision.rs src-tauri/src/lib.rs
git commit -m "feat: supervise visible night boost lifecycle"
```

### Task 4: Honest controls, settings, and unique candidate identity

**Files:**
- Modify: `src/night-vision/main.ts`
- Modify: `src/night-vision/style.css`
- Modify: `src/lib/api.ts`
- Modify: `src/main/settings/Settings.svelte`
- Modify: `src/lib/i18n/vi.ts`
- Modify: `src/lib/i18n/en.ts`
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes the expanded camelCase `NightVisionState`
- Displays build fingerprint, visual-filter status, and gamma status separately

- [ ] **Step 1: Update the TypeScript state contract before rendering changes**

Add `visualBoostReady`, `visualBoostApplied`, `gammaApplied`, and `buildFingerprint`. Keep the button off/waiting unless `visualBoostApplied` is true; a gamma warning must not say the feature is unavailable when the visual layer works.

- [ ] **Step 2: Make the in-game control obvious**

Change the window constants to `190 x 48` and margin 16, then update CSS with a two-pixel high-contrast border and clearly distinct off, waiting, on, and error colors. Keep the same click and `Ctrl+Alt+N` paths.

- [ ] **Step 3: Update settings copy and identity**

Show visual layer and gamma as separate rows. Vietnamese copy explains that higher strength makes darkness brighter but can make colors pale. Set package, Cargo, lockfile, and Tauri versions to `1.7.1`; show the Rust build fingerprint in Settings.

- [ ] **Step 4: Verify frontend, configuration, and version consistency**

Run `npm run check`, `npm run build`, all release configuration tests, and `cargo test --lib`.

- [ ] **Step 5: Commit Task 4**

```powershell
git add -- src/night-vision src/lib/api.ts src/main/settings/Settings.svelte src/lib/i18n package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src-tauri/src/night_vision.rs
git commit -m "feat: expose honest night boost controls"
```

### Task 5: Production build and installed-runtime qualification

**Files:**
- Modify: `README.md`
- Modify: `README.en.md`
- Create: `docs/verification/night-boost-v1.7.1.md`

**Interfaces:**
- Produces a local NSIS installer and SHA256 evidence
- Does not publish until Task 6 passes

- [x] **Step 1: Run the complete automated gate**

Run formatting check, Svelte check, Vite production build, all Rust tests, safety tests, release config tests, Clippy with warnings denied, and `tauri build`.

- [x] **Step 2: Record and verify candidate identity**

Record the git commit, executable path/hash, installer path/hash, version 1.7.1, and build fingerprint. Confirm the installer contains `night-vision-filter.html` and the installed executable matches the candidate hash.

- [x] **Step 3: Install locally without closing the game**

Stop only TheIsle Overlay, run the v1.7.1 installer silently, restart the installed overlay, and verify The Isle remains running. Do not terminate, restart, or modify the game.

- [x] **Step 4: Exercise runtime state and recovery**

With the game foreground, toggle on and require `visualBoostApplied = true`, `night-vision-filter` visible, matching request/strength logs, and the expected v1.7.1 fingerprint. Alt+Tab and require the filter hidden plus exact gamma restoration; return and require reapplication. Take paired ten-second ON and OFF samples: enabled CPU must remain below 1% of total machine CPU, while the ON-minus-OFF deltas must remain below 96 MiB working set and 32 MiB private memory. Record absolute working set without treating the pre-existing WebView baseline as Night Vision cost.

- [x] **Step 5: Document evidence and commit**

Write FACT/EVIDENCE/UNKNOWN results, keeping visual effectiveness UNKNOWN until the user's gate. Update both READMEs with usage, safety boundary, contrast tradeoff, and `Ctrl+Alt+N` recovery instructions. Commit as `docs: qualify night boost candidate`.

### Task 6: Mandatory user visual acceptance and public release

**Files:**
- Modify: `docs/verification/night-boost-v1.7.1.md`
- Modify: public release metadata only after acceptance

**Interfaces:**
- Consumes explicit user confirmation: `thấy sáng rõ hơn`
- Produces the final v1.7.1 public release

- [ ] **Step 1: Ask for one real-scene check only after all machine gates pass**

Tell the user where the button is, set strength 70, and ask them to compare OFF versus ON in the current night scene.

- [ ] **Step 2: Tune locally until accepted**

If too dark, increase the alpha mapping in one measured increment and repeat automated opacity tests plus installed comparison. If washed out, reduce it in one increment and repeat. Never claim pass from logs or process state.

- [ ] **Step 3: Record explicit acceptance**

Add the user's exact acceptance result and timestamp to the verification document. If the user does not see a clear difference, status remains PARTIAL and no release occurs.

- [ ] **Step 4: Publish only after PASS**

Run the full gate again, hash the final installer, push commits, create tag and public release v1.7.1, link the original upstream repository, and mark v1.7.0 superseded without deleting it.
