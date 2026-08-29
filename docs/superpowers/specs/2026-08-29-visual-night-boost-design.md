# Visual Night Boost v1.7.1 Design

**Date:** 2026-08-29

**Target release:** 1.7.1

**Status:** Proposed design, awaiting user review

**Supersedes:** The gamma-only architecture in the v1.7.0 night-vision design

## Problem and acceptance truth

The v1.7.0 gamma implementation passed API readback, restoration, unit tests,
and installed-runtime checks, but it did not make the user's real night scene
visibly brighter. Driver acceptance is therefore diagnostic evidence only; it
is not visual acceptance.

This design succeeds only when the installed candidate produces an obvious
change in the user's current dark game scene. A running process, an `ON` label,
a painted test page, or a successful gamma readback cannot substitute for that
human acceptance gate.

## Goals

- Make the game image clearly brighter at night with a visible, deterministic
  fallback that does not depend on the display driver honoring gamma changes.
- Keep one in-game button and the existing `Ctrl+Alt+N` hotkey for both the
  visual fallback and the supplemental gamma adjustment.
- Keep the visual layer click-through and non-activating so it cannot steal
  mouse or keyboard control from The Isle.
- Hide the visual layer and restore the original gamma on Alt+Tab, game exit,
  overlay exit, or updater relaunch.
- Report `ON` only after the visual layer has painted and Windows reports its
  window visible.
- Give the candidate a unique version and build fingerprint so another local
  build cannot be mistaken for the tested one.

## Non-goals and safety boundary

The feature will not read or write game memory, inject code, hook DirectX,
capture the screen, inspect game pixels, intercept packets, synthesize input,
modify game files, or use the network. It will not automate NVIDIA Freestyle or
claim to reconstruct detail from pixels that the game renders as pure black.

The implementation owns only overlay windows and uses documented window and
display APIs. Existing Easy Anti-Cheat safety assertions remain mandatory.

## Selected architecture

The gamma ramp remains as a supplemental enhancement. A new static,
full-client visual filter provides the deterministic fallback.

### Visual filter window

The application adds an isolated `night-vision-filter` WebView backed by a
static HTML, TypeScript, and CSS entry point. The window is created hidden with
these properties:

- transparent, undecorated, excluded from the taskbar, and always on top;
- click-through for its entire lifetime;
- unable to take keyboard focus or become the active window;
- sized and positioned to the exact The Isle client rectangle;
- ordered above the game and below the minimap, navigation HUD, compass, and
  Night Vision button.

The existing foreground supervisor owns its geometry, visibility, and z-order.
It shows the filter only when Night Vision is requested and The Isle is the
foreground application. Each supervisor pass repairs a changed client
rectangle or z-order without reading anything from inside the game.

### Deterministic brightness layer

The filter paints a uniform soft green-white layer, `rgb(235, 240, 230)`, with
alpha derived from the configured strength:

```text
strength = 0:       alpha = 0
strength = 1..100: alpha = 0.05 + 0.0025 * strength
```

This gives alpha `0.225` at the default strength 70 and `0.30` at strength 100.
At strength 70, an originally black pixel is composited to approximately
`(53, 54, 52)` on an 8-bit display. The result is therefore visibly different
even when the driver ignores gamma. The tradeoff is reduced contrast and a
slightly washed image; the filter cannot create scene detail that was never
rendered.

The existing gamma curve uses the same strength value. A gamma failure becomes
a warning while the visual filter remains available; it does not disable a
working visual fallback.

## Truthful state model

External Night Vision state adds these facts:

- `requested`: the user's desired toggle state;
- `visualBoostReady`: the filter WebView loaded and registered its listener;
- `visualBoostApplied`: the requested strength was painted and the filter
  window passed visibility readback;
- `gammaApplied`: gamma readback matched the requested curve;
- `lastError`: the most recent filter or gamma failure, if any;
- `buildFingerprint`: the exact candidate identity shown in Settings and logs.

After applying a new strength, the filter waits for two animation frames and
then emits a painted acknowledgement containing the applied strength and a
monotonic request identifier. Rust accepts only the latest matching
acknowledgement and then verifies that the window is visible.

The button may display `NHÌN ĐÊM: BẬT` only when `requested` and
`visualBoostApplied` are both true. Loading, stale acknowledgements, hidden
windows, or window errors display `CHƯA BẬT` with a short reason; they never
produce a false `ON` state. `gammaApplied = false` is shown as a supplemental
gamma warning and does not override a successfully painted visual layer.

## Lifecycle and recovery

1. Create the filter hidden during overlay startup.
2. When the user requests Night Vision while the game is foreground, set its
   bounds and z-order, paint the requested strength, show it, and wait for the
   matching painted acknowledgement plus visibility readback.
3. When The Isle loses foreground focus, immediately hide the filter and
   restore the original gamma while preserving `requested` for automatic
   reapplication on return.
4. When the main overlay becomes foreground, keep the filter hidden so it does
   not tint Settings.
5. On game exit, `ExitRequested`, panic recovery, or updater relaunch, hide the
   filter and restore gamma before teardown.
6. If the filter window crashes or misses its acknowledgement, recreate it once
   and retry the latest request. A second failure leaves Night Vision visibly
   unavailable until the next explicit toggle or application restart.

## Button and settings

The in-game button becomes `190 x 48` logical pixels, stays within 16 pixels of
the game's top-right client edge, and uses a high-contrast border and state
color. Clicking it and pressing `Ctrl+Alt+N` call the same toggle path.

Settings keeps the 0–100 strength slider and states plainly that higher values
raise dark areas but wash out contrast. Settings also shows the active version,
build fingerprint, visual-filter state, and supplemental gamma state.

## Performance and safety

The filter is a static composited window. It has no capture pipeline, image
analysis, canvas animation, or continuous JavaScript rendering loop. Work is
limited to state changes, foreground supervision, geometry repair, and painted
acknowledgements.

Safety tests must reject newly introduced capture APIs, game-process handles,
memory access, injection, hooks, input synthesis, and unreviewed native FFI.
Installed acceptance includes a 10-second sample while the filter is enabled:
the overlay process group must average below 1% of total machine CPU and remain
below 180 MiB combined working set on this machine. The game must remain
running throughout the sample.

## Verification and acceptance gates

### Automated checks

- Unit-test the opacity curve for exact values at 0, 1, 70, and 100, monotonic
  output, and the 0–0.30 bounds.
- Test filter creation as hidden, click-through, non-activating, and excluded
  from the taskbar.
- Test exact client-bound synchronization and the ordering
  `game < filter < HUD/minimap/button`.
- Test that `ON` requires the latest painted acknowledgement and successful
  visibility readback.
- Test Alt+Tab hiding, foreground reapplication, game-exit cleanup, stale
  acknowledgement rejection, and one-shot recreation after failure.
- Run the existing anti-cheat safety suite, all Rust unit tests, frontend
  checks, release configuration tests, Clippy, and the production build.

### Installed-runtime checks

- Install the local v1.7.1 candidate and verify its version, executable hash,
  installer hash, and build fingerprint.
- Verify the expected controller, filter, minimap, compass, and button windows
  without relying on their mere existence as visual proof.
- Toggle Night Vision and require the matching `visualBoostApplied`
  acknowledgement while The Isle remains running.
- Alt+Tab and require the filter to become hidden and gamma recovery to match
  the original ramp; return to the game and require clean reapplication.
- Measure the 10-second CPU and memory acceptance thresholds with Night Vision
  enabled.

### Mandatory human dark-scene gate

The user tests the installed candidate in the actual current night scene at
strength 70 and must explicitly confirm `thấy sáng rõ hơn`. If the result is
still too dark, adjust the local candidate's mapping and retest. If it is too
washed out or glaring, reduce the mapping and retest.

No public release, final handoff, or claim of completion is allowed until this
explicit visual confirmation is received. This gate cannot be replaced by
screenshots that fail to capture, automated status, logs, hashes, or tests.

## Release sequence

1. Build and install a uniquely identified local v1.7.1 candidate.
2. Complete automated and installed-runtime checks.
3. Obtain the user's explicit dark-scene acceptance.
4. Only then commit the release evidence, update user documentation, build and
   hash the final installer, tag v1.7.1, and publish it to the public fork.
5. Mark v1.7.0 as superseded in documentation without deleting its release or
   evidence.

## Risks and tradeoffs

- A bright veil raises the black floor but compresses contrast; the strength
  slider and human gate control that tradeoff.
- Pure-black source pixels become visible gray-green, but missing scene detail
  cannot be recovered.
- A full-client transparent WebView adds DWM/GPU composition cost; installed
  performance sampling is required before release.
- Windows z-order can change during focus transitions; the supervisor repairs
  and verifies it while the game is foreground.
- Display drivers may still ignore gamma, so gamma remains supplemental and is
  reported separately from the deterministic visual layer.
