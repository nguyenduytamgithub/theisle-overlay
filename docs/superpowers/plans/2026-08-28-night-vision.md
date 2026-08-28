# Night Vision 1.7.0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a visibly effective, reversible, anti-cheat-safe Night Vision mode controlled by an in-game button, `Ctrl+Alt+N`, and Settings.

**Architecture:** A Rust-owned controller is the only component allowed to touch the Windows display gamma ramp. Pure curve, comparison, lifecycle, and recovery-record logic stay testable without hardware; a narrow Win32 adapter identifies the game monitor and performs read/apply/restore. A small non-activating Tauri webview displays truthful state and calls the same backend command as the hotkey and Settings.

**Tech Stack:** Rust 2021, Tauri 2, `windows` 0.62 GDI/Win32 APIs, Serde JSON, TypeScript/Vite, Svelte 5 Settings UI, NSIS installer.

## Global Constraints

- Target release is exactly `1.7.0`.
- Default hotkey is exactly `Ctrl+Alt+N` and uses the existing rebinding system.
- Persist `strength = 70` and `show_button = true`; requested ON/OFF state is session-only and always starts OFF.
- Never read/write game memory, inject, hook DirectX, capture packets or frames, synthesize input, or modify game files.
- Apply gamma only to the monitor containing the foreground The Isle game window.
- Restore the exact captured ramp on OFF, Alt-Tab, game exit, monitor switch, tray Quit, and recovery from a previous unclean app exit.
- Report ON only after driver readback verifies the applied ramp within the documented tolerance.
- No release-complete claim until automated tests, machine gamma round-trip, installed runtime, focus/restore behavior, and a real dark-scene user check have evidence.

---

### Task 1: Pure gamma curve and verification rules

**Files:**
- Create: `src-tauri/src/night_vision/curve.rs`
- Create: `src-tauri/src/night_vision.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `pub type GammaRamp = [[u16; 256]; 3]`
- Produces: `pub(crate) fn lifted_ramp(strength: u8) -> GammaRamp`
- Produces: `pub(crate) fn ramps_match(expected: &GammaRamp, actual: &GammaRamp, tolerance: u16) -> bool`
- Produces: `pub(crate) const READBACK_TOLERANCE: u16 = 257`
- Consumes: no Tauri or Win32 state.

- [ ] **Step 1: Add failing curve tests**

Add tests in `src-tauri/src/night_vision/curve.rs` that iterate every strength `0..=100` and assert all three channels are identical, every channel is monotonic, index `0 == 0`, index `255 == u16::MAX`, and strength 70 raises index 128 to at least `42_598` while leaving index 192 below `u16::MAX`. Add comparison tests proving a difference of `257` passes and `258` fails.

```rust
#[test]
fn strength_70_lifts_midtones_without_clipping_highlights() {
    let ramp = lifted_ramp(70);
    assert!(ramp[0][128] >= 42_598);
    assert!(ramp[0][192] < u16::MAX);
    assert_eq!(ramp[0], ramp[1]);
    assert_eq!(ramp[1], ramp[2]);
}
```

- [ ] **Step 2: Verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml night_vision::curve -- --nocapture`

Expected: compilation fails because `night_vision::curve`, `lifted_ramp`, and `ramps_match` do not exist.

- [ ] **Step 3: Implement the minimal neutral curve**

Use a power curve with `gamma = 1.0 - 0.65 * clamped_strength / 100.0`, round each normalized sample back to `u16`, and explicitly assign the two endpoints. Duplicate the channel into RGB rather than applying three different colour curves.

```rust
pub(crate) fn lifted_ramp(strength: u8) -> GammaRamp {
    let strength = strength.min(100) as f64 / 100.0;
    let gamma = 1.0 - 0.65 * strength;
    let channel = std::array::from_fn(|index| {
        if index == 0 { return 0; }
        if index == 255 { return u16::MAX; }
        (((index as f64 / 255.0).powf(gamma) * u16::MAX as f64).round()) as u16
    });
    [channel, channel, channel]
}
```

- [ ] **Step 4: Verify GREEN and commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml night_vision::curve -- --nocapture`

Expected: all new curve tests pass.

Commit: `git commit -m "feat: add verified night vision gamma curve"`

---

### Task 2: Recovery record and Win32 display adapter

**Files:**
- Create: `src-tauri/src/night_vision/recovery.rs`
- Create: `src-tauri/src/night_vision/windows.rs`
- Modify: `src-tauri/src/night_vision.rs`
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/bin/verify_night_vision.rs`

**Interfaces:**
- Consumes: `GammaRamp`, `ramps_match`, `settings::GAME_PROCESS_NAME`, and existing `win::game_window::find_game_window`.
- Produces: `RecoveryRecord { display_name: String, ramp: Vec<u16> }` with strict 768-entry validation.
- Produces: `DisplayGamma::for_game_window(hwnd: isize) -> Result<DisplayGamma, NightVisionError>`.
- Produces: `DisplayGamma::{read, apply_verified, restore}` and `restore_recovery_record(path: &Path)`.
- Produces: `settings::night_vision_recovery_path() -> PathBuf`.

- [ ] **Step 1: Add failing recovery tests**

Test a complete 768-entry round trip, rejection of 767/769 entries, atomic-write replacement, and preservation of non-ASCII display IDs. Use a unique directory under `std::env::temp_dir()` and remove only that exact test directory after success.

```rust
#[test]
fn recovery_round_trip_preserves_every_entry() {
    let expected = RecoveryRecord::from_ramp(r"\\.\DISPLAY1", &fixture_ramp());
    write_atomic(&path, &expected).unwrap();
    let actual = read_validated(&path).unwrap();
    assert_eq!(actual.display_name, expected.display_name);
    assert_eq!(actual.to_ramp().unwrap(), fixture_ramp());
}
```

- [ ] **Step 2: Verify recovery RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml night_vision::recovery -- --nocapture`

Expected: compilation fails because the recovery types and functions are absent.

- [ ] **Step 3: Implement recovery persistence**

Serialize `{ version: 1, display_name, ramp }` to a sibling `.tmp`, flush it, then `std::fs::rename` it over `night-vision-recovery.json`. Validate `version == 1` and `ramp.len() == 768` before any Win32 restore call. Delete the record only after a verified successful restore.

- [ ] **Step 4: Add failing adapter contract tests**

Keep Win32 calls behind a `GammaApi` trait whose fake implementation records open/read/set/delete operations. Test that `apply_verified` writes the recovery record before `SetDeviceGammaRamp`, reads back immediately, restores on mismatch, and never deletes the record after a failed restore.

```rust
trait GammaApi: Send {
    fn read(&mut self, display: &str) -> Result<GammaRamp, NightVisionError>;
    fn write(&mut self, display: &str, ramp: &GammaRamp) -> Result<(), NightVisionError>;
}
```

- [ ] **Step 5: Verify adapter RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml night_vision::windows -- --nocapture`

Expected: compilation fails because `GammaApi`, `DisplayGamma`, and verified apply/restore are absent.

- [ ] **Step 6: Implement the Win32 adapter**

Use `MonitorFromWindow`, `GetMonitorInfoW`, `CreateDCW`, `GetDeviceGammaRamp`, `SetDeviceGammaRamp`, and `DeleteDC`. The adapter owns no game handle other than the already detected top-level HWND and opens only a display DC named by `MONITORINFOEXW.szDevice`. Every DC is released through a small RAII wrapper.

Add `Win32_Graphics_Gdi`/`Win32_UI_WindowsAndMessaging` APIs only; do not add process-memory, injection, capture, or input features. The probe binary must read the original, apply strength 70, verify entries 32/64/128 and readback, restore the exact original, verify restoration, then exit non-zero on any failed gate.

- [ ] **Step 7: Verify GREEN and commit**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml night_vision -- --nocapture
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
```

Expected: all recovery/adapter tests pass and every target compiles.

Commit: `git commit -m "feat: add crash-safe Windows gamma adapter"`

---

### Task 3: Controller, foreground supervisor, commands, hotkey, and exit restoration

**Files:**
- Modify: `src-tauri/src/night_vision.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/hotkeys.rs`
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/tray.rs`

**Interfaces:**
- Consumes: `DisplayGamma`, `GammaApi`, `game_window::{find_game_window,is_iconic,is_foreground,client_rect_on_screen}`, and `events::emit_all`.
- Produces: serializable `NightVisionState { requested, applied, supported, strength, error_key }`.
- Produces Tauri commands `get_night_vision_state`, `toggle_night_vision`, and `set_night_vision_strength`.
- Produces helpers `night_vision::toggle_from_app(app)`, `night_vision::restore_before_exit(app)`, and `night_vision::create(app)`.

- [ ] **Step 1: Add failing controller state-machine tests**

With a fake gamma device, test these exact transitions: startup OFF; toggle while game active applies; Alt-Tab restores but keeps `requested = true`; refocus reapplies; monitor change restores old monitor before applying new; toggle OFF restores and clears request; driver readback failure yields `supported = false`, `applied = false`, and an error key.

- [ ] **Step 2: Verify controller RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml night_vision::tests -- --nocapture`

Expected: compilation fails because `NightVisionController` and transition methods are absent.

- [ ] **Step 3: Implement controller and 250 ms supervisor**

Manage `NightVisionController` as Tauri state. The controller mutex owns requested/applied/support/strength/current-display state; the supervisor polls the existing game-window helpers, debounces two missed/unfocused ticks, applies only when `requested && game_active`, and restores on every other path. Emit `night-vision://changed` only when the public state changes.

Do not persist `requested`. Read persisted strength once at startup and update it through `set_night_vision_strength`; reapply immediately when active.

- [ ] **Step 4: Add failing settings/hotkey tests**

Extend legacy settings merge tests to require:

```rust
assert_eq!(merged["night_vision"]["strength"], 70);
assert_eq!(merged["night_vision"]["show_button"], true);
assert_eq!(merged["hotkeys"]["toggle_night_vision"], "Ctrl+Alt+N");
```

Extend hotkey default parsing tests with `Ctrl+Alt+N`.

- [ ] **Step 5: Verify settings/hotkey RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml settings::tests hotkeys::tests -- --nocapture`

Expected: assertions fail because the defaults are absent.

- [ ] **Step 6: Wire commands, hotkey, startup recovery, and Quit**

Register the module, managed controller, three commands, and `night_vision::create` in `lib.rs`. Restore a surviving recovery record synchronously after `settings::ensure_dirs()` and before Night Vision can be enabled. Add `toggle_night_vision` to the hotkey dispatch match. In the tray `quit` branch, call `restore_before_exit(app)` before `app.exit(0)`. Implement `Drop` restoration as a secondary normal-exit guard.

- [ ] **Step 7: Verify GREEN and commit**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
```

Expected: all Rust tests and targets pass.

Commit: `git commit -m "feat: control night vision across game focus"`

---

### Task 4: Clickable in-game button webview

**Files:**
- Create: `night-vision.html`
- Create: `src/night-vision/main.ts`
- Create: `src/night-vision/style.css`
- Modify: `vite.config.ts`
- Modify: `src-tauri/src/night_vision.rs`
- Modify: `src-tauri/capabilities/default.json`

**Interfaces:**
- Consumes commands/events from Task 3.
- Emits `night-vision://ready` after listeners are registered.
- Produces button states `NHÌN ĐÊM: TẮT`, `NHÌN ĐÊM: BẬT`, and `KHÔNG HỖ TRỢ` with English equivalents.

- [ ] **Step 1: Add failing backend window-policy tests**

Extract and test pure `button_should_show(show_button, game_active, main_in_front)` and `button_anchor(game_rect, scale, size, margin)` functions. Assert the button is hidden when the game is not foreground, Settings is foreground, or `show_button` is false; assert top-right coordinates remain inside the game client rectangle.

- [ ] **Step 2: Verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml night_vision::button -- --nocapture`

Expected: compilation fails because the button policy helpers do not exist.

- [ ] **Step 3: Implement the non-activating window and UI**

Build label `night-vision`, URL `night-vision.html`, logical size `164 x 42`, transparent, undecorated, non-resizable, hidden, always-on-top, skip-taskbar, `focused(false)`, and `focusable(false)`. Call the existing overlay style assertion to force `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`, but do not enable click-through. Recreate a dead webview after a 5-second throttle and anchor it at the game client top-right.

The frontend registers listeners before its first state fetch, invokes only the shared Rust command, disables itself while a request is in flight, and renders ON only from backend `applied = true`.

- [ ] **Step 4: Authorize/build and verify UI**

Add `nightVision` as a Vite input and add window label `night-vision` to the existing capability. Run:

```powershell
npm run check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml night_vision::button -- --nocapture
```

Expected: Svelte/TypeScript checks pass, Vite emits the new entry, and button-policy tests pass.

- [ ] **Step 5: Commit**

Commit: `git commit -m "feat: add clickable in-game night vision control"`

---

### Task 5: Settings, translations, safety test, documentation, and 1.7.0 version

**Files:**
- Modify: `src/lib/api.ts`
- Modify: `src/main/settings/Settings.svelte`
- Modify: `src/main/settings/HotkeyEditor.svelte`
- Modify: `src/main/guide/Guide.svelte`
- Modify: `src/lib/i18n/vi.ts`
- Modify: `src/lib/i18n/en.ts`
- Create: `src-tauri/tests/night_vision_safety.rs`
- Modify: `README.md`
- Modify: `README.en.md`
- Create: `docs/releases/v1.7.0-night-vision.md`
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: `NightVisionState` and the three commands/events from Task 3.
- Produces: Settings toggle, strength slider `0..100`, show-button checkbox, verification/error status, and hotkey editor row.

- [ ] **Step 1: Add the failing anti-cheat boundary test**

Create a source-policy test that allows only the named display GDI APIs in `night_vision` files and fails if those files contain game-process opening, process-memory, remote-thread, hook, capture, packet, or synthetic-input API names such as `OpenProcess`, `ReadProcessMemory`, `WriteProcessMemory`, `CreateRemoteThread`, `SetWindowsHookEx`, `BitBlt`, `SendInput`, or `mouse_event`.

- [ ] **Step 2: Verify safety RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test night_vision_safety -- --nocapture`

Expected: test target fails because the safety test file/required policy is not yet present.

- [ ] **Step 3: Add Settings and bilingual copy**

Add TypeScript state and API wrappers, subscribe to `night-vision://changed`, and render:

- Vietnamese: `Nhìn đêm`, `Bật/Tắt nhìn đêm`, `Cường độ`, `Hiện nút trong game`, `Đã áp dụng`, `Đang chờ game`, and localized driver/HDR failure text.
- English: exact equivalents under the same keys.

The slider calls `set_night_vision_strength`; checkbox uses the existing settings patch path; Settings toggle calls `toggle_night_vision`. Add `toggle_night_vision` to both the editor action list and Guide hotkey table.

- [ ] **Step 4: Update public docs and version metadata**

Set every first-party version field to `1.7.0`, regenerate lockfiles through normal package/Cargo commands, and document the display-only safety boundary, SDR/HDR limitation, focus restore, crash recovery, button/hotkey usage, source attribution to upstream v1.5.2, and the fact that this is direct visibility enhancement rather than game route or memory access.

- [ ] **Step 5: Verify GREEN and commit**

Run:

```powershell
npm run check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
```

Expected: all checks pass and the safety test explicitly lists zero forbidden matches.

Commit: `git commit -m "docs: release night vision 1.7.0"`

---

### Task 6: Machine proof, installer, installed runtime, and user acceptance

**Files:**
- Update after measured build: `docs/releases/v1.7.0-night-vision.md`
- Produce: `src-tauri/target/release/bundle/nsis/TheIsle Overlay_1.7.0_x64-setup.exe`

**Interfaces:**
- Consumes: completed 1.7.0 source and the current RTX 3060/Windows display stack.
- Produces: exact test output, gamma probe evidence, SHA-256, installer, installed executable version, runtime logs, focus restore evidence, and a dark-scene acceptance result.

- [ ] **Step 1: Run the fresh full verification suite**

```powershell
npm run check
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: every command exits 0 with no ignored failure.

- [ ] **Step 2: Run the guarded machine gamma probe**

With The Isle visible on the intended monitor, run the probe. It must print the display ID, original/readback values at entries 32/64/128/255, verified strength-70 values, and exact-original restoration. If any restore check fails, immediately rerun restore-only mode and stop release work.

- [ ] **Step 3: Build and hash the NSIS installer**

Run: `npm run tauri build`

Then record installer byte size and `Get-FileHash -Algorithm SHA256` in the release note.

- [ ] **Step 4: Install without disturbing user data**

Close only the running TheIsle Overlay process after calling its Night Vision restore path; do not stop Steam or The Isle. Run the NSIS installer in current-user mode, launch the installed app, and confirm installed file/product version `1.7.0`.

- [ ] **Step 5: Verify installed runtime behavior**

Confirm:

- button appears only when The Isle is foreground and accepts a click without stealing game keyboard focus;
- button and `Ctrl+Alt+N` converge on one state;
- strength changes reapply and read back;
- Alt-Tab restores the original ramp within 500 ms while requested state remains latched;
- returning to the game reapplies;
- OFF and tray Quit restore exactly;
- no persistent capture/render loop exists and FPS shows no repeatable regression above measurement noise.

- [ ] **Step 6: Perform the dark-scene acceptance gate**

Ask the user to enter an actually dark in-game area and confirm terrain/objects that were previously indistinguishable become clearly visible at strength 70. If too weak, tune the curve under a new failing midpoint/upper-quarter test, rebuild, reinstall, and repeat. If the driver/HDR path rejects gamma, report `KHÔNG HỖ TRỢ` truthfully and document NVIDIA Freestyle as the manual fallback.

- [ ] **Step 7: Final repository verification and commit measured evidence**

Run `git status --short`, verify only intended files/artifacts are present, update the release note with actual commands/results, and commit the evidence note without committing the binary installer.

Commit: `git commit -m "test: verify installed night vision 1.7.0"`
