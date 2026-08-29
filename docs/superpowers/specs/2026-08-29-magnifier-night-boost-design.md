# Magnifier Night Boost v1.7.2 Design

**Status:** Approved by the user's standing instruction to use any effective
method, self-test it on the live game, and continue without another design
choice prompt.

**Supersedes:** `2026-08-29-visual-night-boost-design.md` and the unreleased
v1.7.1 flat-tint candidate.

## Problem and measured root cause

The v1.7.1 filter paints a constant pale color over the game. A real live-scene
OFF/ON capture proved that it changes the appearance but not in a useful way:

| Sampled play area | Mean luminance | Luminance standard deviation |
|---|---:|---:|
| OFF | 1.56 | 3.43 |
| v1.7.1 flat tint ON | 54.43 | 1.13 |

The tint raises black to gray while compressing local contrast, so foliage and
the dinosaur remain hard to distinguish. A painted acknowledgement proves only
that the flat layer exists; it cannot prove useful night visibility.

## Options considered

1. **NVIDIA Freestyle/Game Filter.** This is the best vendor-native path and it
   opens in The Isle through `Alt+F3`, but GeForce Experience exposes no stable
   supported API for the Overlay button to select or toggle a preset. It remains
   a manual fallback, not the application implementation.
2. **Windows Magnification color transform — selected.** A click-through
   magnifier control copies the already composed game region and applies a
   per-channel gain matrix. A throwaway gain-4 prototype changed the live scene
   from nearly black to clearly visible foliage, terrain, and dinosaur while
   the in-game counter showed 76 FPS.
3. **Windows Graphics Capture/Desktop Duplication plus a custom Direct3D
   shader.** This supports nonlinear curves but adds substantially more code,
   GPU synchronization, latency, and failure modes than this requirement needs.

## Architecture

Keep the existing `night-vision-filter` Tauri top-level window because it
already owns game-bound geometry, focus supervision, click-through behavior,
z-order, the in-game button relationship, and recovery. Replace its flat CSS
paint with a native Windows `Magnifier` child control created inside that host.

A focused Windows module owns all Magnification API calls:

- initialize the Magnification runtime once;
- create/destroy the child control for a supplied host HWND;
- set a 1.0 identity spatial transform and exact desktop-coordinate source
  rectangle matching the game client area;
- apply a diagonal RGB gain matrix derived from strength;
- exclude the host and every Overlay-owned HWND from the magnified source;
- expose readback facts for child existence, source rectangle, and color
  effect; and
- destroy the child before the host is recreated or the app exits.

The host WebView becomes fully transparent and no longer claims that its own
paint is the visual effect. `visualBoostApplied` becomes true only after native
creation/configuration succeeds, readback matches the current source rectangle
and gain, and the host window is visible.

## Strength mapping

Strength 0 disables the magnifier. Strength 1–100 maps monotonically to RGB gain
`1.0 + 4.0 * strength / 100`, producing:

- 1 → 1.04x;
- 50 → 3.00x;
- 70 → 3.80x; and
- 100 → 5.00x.

The matrix has no additive offset. Black stays black, while differences between
dark pixels expand until highlights clip. This is intentionally the opposite
of the failed flat tint, which added gray and reduced contrast.

Windows display gamma is no longer applied for normal Night Vision operation.
Startup still restores a stale pre-v1.7.2 gamma recovery record if one exists.

## Runtime behavior

- The `NHÌN ĐÊM` button and `Ctrl+Alt+N` keep their current interface.
- When requested and The Isle is foreground, synchronize host/child geometry,
  source rectangle, exclusion list, and gain, then show the host below the HUD,
  minimap, and button.
- On Alt-Tab, game minimization/exit, OFF, or app exit, hide the host and destroy
  the magnifier child. Re-entering the game creates a fresh child from the
  latest settings.
- Any native failure leaves the button in a truthful unavailable/waiting state
  with a logged Windows error. It must not fall back to the gray tint.
- Exclusive Fullscreen remains unsupported; Windowed or Borderless is required.

## Safety boundary

The new implementation uses the Microsoft Magnification API to read displayed
screen pixels and redraw a transformed local view. Documentation must state
this plainly. It does not open the game process, read game memory, inject DLLs,
hook DirectX, synthesize input, capture packets, save or transmit frames, or
share player position. The source filter excludes all Overlay windows to avoid
recursive capture and duplicate UI.

This is a functional and safety change from v1.7.1: the previous claim of “no
capture” no longer applies. No claim that a third-party server or anti-cheat
operator has approved the feature may be made without their own confirmation.

## Verification

### Automated

- Unit-test exact gain values, monotonicity, and the 1.0–5.0 bounds.
- Unit-test strength 0 as disabled.
- Test that the Windows adapter configures identity spatial transform, exact
  source rectangle, diagonal color matrix, and overlay exclusion list.
- Test lifecycle decisions for ON, OFF, Alt-Tab, game exit, host recreation,
  stale child cleanup, and native failure.
- Update the anti-cheat safety test to allow only the reviewed Magnification API
  entrypoints and continue rejecting game handles, memory APIs, injection,
  DirectX hooks, input synthesis, networking, frame saving, and transmission.
- Run Rust tests, Svelte checks, production Vite build, Clippy with warnings
  denied, and the NSIS production build.

### Installed live game

- Install v1.7.2 without stopping The Isle.
- Capture OFF and ON frames from the same live night scene.
- Require ON mean luminance to exceed OFF by at least 3x and ON luminance
  standard deviation to exceed OFF by at least 1.5x in a HUD-free play-area
  sample; this proves both brightness and detail contrast increased.
- Confirm the dinosaur/foliage/terrain are visibly distinguishable in the ON
  capture, the host remains click-through, the source follows game bounds, and
  Alt-Tab/OFF removes the transformed view.
- Measure FPS shown by the game and a paired ten-second process sample. Record
  rather than hide any latency, CPU, memory, or FPS regression.

### Human gate

Public push/tag/release remains blocked until the user sees the installed
v1.7.2 ON/OFF result in the real night scene and explicitly accepts it.
