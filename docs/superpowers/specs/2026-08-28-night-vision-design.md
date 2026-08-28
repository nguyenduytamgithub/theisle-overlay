# Anti-Cheat-Safe Night Vision Design

**Date:** 2026-08-28  
**Target release:** 1.7.0  
**Status:** Approved design, awaiting implementation-plan review

## Goal

Add a visibly effective night-visibility mode for The Isle that the player can
toggle from a small button directly over the game or with `Ctrl+Alt+N`. The
feature must have negligible frame-time cost and must preserve the overlay's
Easy Anti-Cheat safety boundary.

## Non-goals and safety boundary

Night Vision will not read or write game memory, inject a DLL, hook DirectX,
capture packets, synthesize game input, or modify game files. It will not use a
full-screen screen-capture/re-render pipeline. The feature changes only the
display device's hardware gamma table through documented GDI calls and manages
only windows owned by TheIsle Overlay.

No claim of universal HDR support will be made. Windows documents gamma-ramp
limitations and undefined interaction with HDR/color-management software. A
driver that rejects or ignores the requested curve must produce a visible error
state rather than a false ON state.

## Approaches considered

### 1. Hardware gamma ramp plus a small native overlay control (selected)

- Changes shadow and midtone output without capturing or re-rendering frames.
- Costs no per-frame GPU work after the 256-entry ramp is installed.
- Can be verified by reading the active ramp back from the driver.
- Affects the selected monitor, so the controller automatically restores the
  original ramp whenever the game loses foreground focus.

### 2. NVIDIA Freestyle

- Can produce excellent per-game image filters.
- Has no stable public API for this app to toggle one preset from its own button.
- Availability is game/profile/driver dependent, so it remains a documented
  manual fallback rather than the primary implementation.

### 3. Bright translucent window over the game

- Easy to implement and game-window scoped.
- Washes out the whole image, cannot recover clipped shadow detail, and adds a
  full-screen composited surface. It is rejected as lower quality and less
  efficient.

## User experience

### In-game button

A dedicated Tauri window named `night-vision` is anchored to the top-right of
the game client area. It is approximately 164 x 42 logical pixels, always on top,
absent from the taskbar, and uses `WS_EX_NOACTIVATE` so clicking it does not take
keyboard focus from The Isle. Unlike the minimap, this small window is not
click-through because its single purpose is to accept the toggle click.

The button has three truthful states:

- `NHÌN ĐÊM: TẮT` / `NIGHT VISION: OFF`
- `NHÌN ĐÊM: BẬT` / `NIGHT VISION: ON`
- `KHÔNG HỖ TRỢ` / `UNAVAILABLE`, with a short reason available in Settings

The button is shown only while The Isle is present and foreground. It never
covers the main overlay window. A Settings option can hide the button for users
who want hotkey-only control.

### Hotkey and settings

`Ctrl+Alt+N` invokes the exact same Rust toggle command as the button. The
existing hotkey editor can rebind it. Settings includes:

- an enable/disable button mirroring the in-game button;
- a strength slider from 0 to 100, default 70;
- a `Show button in game` checkbox, default true;
- current state and driver verification result.

The requested ON/OFF state is session-only and starts OFF after every clean app
launch. Strength, button visibility, and the hotkey persist.

## Gamma controller

`src-tauri/src/night_vision.rs` owns all gamma state. It will:

1. Locate the monitor containing the existing detected game window.
2. Open a display DC for that monitor, not an HDC for the game process.
3. Read and retain the original 3 x 256 `u16` ramp before the first enable.
4. Generate a monotonic, neutral RGB curve that keeps black at 0 and white at
   the device maximum while lifting shadows and midtones. Strength 70 targets a
   midpoint lift of at least 30 percent over the original linear value without
   clipping the upper quarter of the curve.
5. Apply the curve and immediately read it back. ON is reported only when the
   readback matches within the driver's quantization tolerance.
6. Restore the exact original ramp when toggled off, when The Isle loses focus,
   on normal app exit, and before switching monitors.

A 250 ms supervisor shares the same game-presence/foreground rules as the HUD.
The user's requested ON state stays latched during a brief Alt-Tab, but the
hardware effect is removed from the desktop and reapplied only after the game
returns to foreground.

## Crash recovery

Before changing the ramp, the controller atomically stores the original ramp
and display identity under the app's local data directory. A clean restore
removes that recovery record. On the next app launch, a surviving record is
restored before Night Vision can be enabled. This covers a prior forced kill or
panic. Windows display reset/reboot remains an additional recovery path.

The tray Quit action explicitly restores Night Vision before calling
`app.exit(0)`. The managed controller also restores in `Drop` as a second normal
exit path.

## State and events

The controller exposes Tauri commands:

- `get_night_vision_state() -> NightVisionState`
- `toggle_night_vision() -> NightVisionState`
- `set_night_vision_strength(strength: u8) -> NightVisionState`

`NightVisionState` contains requested, applied, supported, strength, and an
optional localized-error key. Every state change emits `night-vision://changed`
to the button, HUD, and Settings UI. Only Rust writes hardware state; frontend
windows are views and command callers.

## Failure handling

- If the display DC or gamma APIs fail, the original ramp remains/restores and
  state becomes UNAVAILABLE.
- If readback does not reflect the requested curve, the controller restores and
  reports that the driver, HDR, or color-management path rejected the change.
- If the game monitor changes, restore the previous monitor before applying to
  the new monitor.
- If the visible effect is too weak during the in-game acceptance check, tune
  the strength-to-curve mapping and repeat the check. Do not ship a no-op.
- NVIDIA Freestyle is documented as the manual fallback when gamma control is
  unavailable; the app will not automate the NVIDIA overlay.

## Testing and acceptance

### Automated tests

- Curve values are monotonic for every strength from 0 through 100.
- Black remains 0, white remains at the original endpoint, RGB channels stay
  neutral, and strength 70 lifts the midpoint by at least 30 percent.
- Strength is clamped to 0 through 100.
- Button/hotkey commands converge on one controller state.
- Foreground transitions restore and reapply without losing requested state.
- Recovery-record round trip preserves all 768 ramp entries and display ID.
- Existing forbidden-API CI accepts the new display-only calls and still rejects
  game-process access, hooks, injection, and synthetic input.

### Machine verification on the current RTX 3060

- `GetDeviceGammaRamp` succeeds before installation.
- Enable, readback, disable, and exact-original restore all succeed.
- The displayed ramp at strength 70 lifts entries 32, 64, and 128 by the target
  amount while preserving endpoint ordering.
- The clickable button appears only over the foreground game and does not steal
  game keyboard focus.
- Button and `Ctrl+Alt+N` toggle the same visible state.
- With an actually dark in-game scene, the user confirms terrain/objects that
  were difficult to distinguish become clearly visible. Increase strength or
  stop and report UNAVAILABLE if this acceptance gate is not met.
- Game FPS is compared before/after; no persistent capture/render loop is added,
  so any repeatable regression above measurement noise blocks release.

## Release and documentation

The public fork will release this together with the navigation HUD as version
1.7.0. README VI/EN will explain the button, hotkey, strength control, SDR/HDR
limitations, anti-cheat boundary, recovery behavior, original upstream source,
and a clear comparison with upstream 1.5.2. The unsigned NSIS installer will be
attached to the GitHub release only after local installation and runtime
verification pass.
