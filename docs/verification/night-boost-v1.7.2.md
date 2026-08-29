# Night Boost v1.7.2 Candidate Verification

**Recorded:** 2026-08-29

**Status:** PARTIAL — source behavior, safety gates, packaging, installation,
native readback, realtime Tab/Back refresh, and paired performance sampling
pass. A fresh matched deep-night OFF/ON capture and human visual acceptance are
still pending.

## Candidate design

### FACT

- Version target: `1.7.2`.
- Build fingerprint target: `1.7.2-magnifier-boost-b`.
- The rejected constant-color v1.7.1 WebView paint path has been removed.
- The existing click-through Tauri window hosts a native Windows Magnification
  child that applies an identity spatial transform and diagonal RGB gain.
- A UI-thread timer reapplies the fixed source rectangle every `16 ms`, matching
  the working prototype and preventing stale DirectX frames after Tab/Alt-Tab.
- Strength maps monotonically from gain `1.0` at 0% to `5.0` at 100%; the 70%
  default requests gain `3.8`.
- Normal operation does not open or apply a display gamma session. Startup only
  restores a stale recovery record left by a pre-v1.7.2 build.

## Safety boundary

### FACT

- Windows Magnification locally reads displayed screen pixels to redraw the
  selected game rectangle. This is screen capture in the ordinary Windows API
  sense and is disclosed as such.
- It does not open or read The Isle process memory, inject a DLL, hook DirectX,
  synthesize game input, access game network traffic, or save/transmit images.
- Windowed or Borderless mode is required. Exclusive Fullscreen is unsupported.
- Server permission remains outside the program's control; users must follow
  the rules of the server they join.

## Source verification

### EVIDENCE

- Full Rust suite: PASS (`103` passed, `0` failed, `9` deliberately ignored
  live/integration tests), plus `0`-test binary target.
- Narrow Win32/safety integration suite: PASS (`4` passed, `0` failed).
- Release/version/copy suite: PASS (`4` passed, `0` failed).
- Clippy with warnings denied: PASS.
- Frontend type check: PASS, `0` errors and `0` warnings.
- Production frontend build: PASS; `night-vision-filter.html` is present and
  contains no flat-color opacity paint protocol.
- Targeted `rustfmt --check` for the changed Rust files: PASS. Repository-wide
  `cargo fmt --check` remains blocked by unrelated pre-existing formatting debt;
  no unrelated files were bulk-reformatted.
- Independent code review after race-condition fixes: no Critical or Important
  findings.

## Candidate artifact

### EVIDENCE

- Installer: `D:\CodexBuild\theisle-overlay-nightboost\release\bundle\nsis\TheIsle Overlay_1.7.2_x64-setup.exe`
- Size: `5,808,233` bytes.
- SHA-256: `713C219772002684C21106E759C71C90422566B2FDD7AA6573D48FCC50CE9354`.
- Tauri release build and NSIS packaging: PASS.

## Installed runtime proof

### EVIDENCE

- Installed executable: `C:\Users\Admin\AppData\Local\TheIsle Overlay\theisle-overlay.exe`.
- Installed version/size/SHA-256: `1.7.2`, `21,471,744` bytes,
  `DA41D52D287B86CD1F716951424F30AAE53C91316B1F8954C8214E777DCD1E07`.
- The Isle stayed alive at PID `36904` across both candidate installations.
- Native readback: gain `3.80`, source `(0, 0, 1920, 1080)`, refresh `16ms`,
  fingerprint `1.7.2-magnifier-boost-b`.
- Realtime regression: with Night Vision ON, Tab changed the magnifier output to
  Status Report; clicking Back restored the current forest scene within one
  second. Diagnostic images are under
  `D:\CodexBuild\theisle-overlay-nightboost\diagnostics\v1.7.2-live-b`.
- The first installed `magnifier-boost-a` candidate is REJECTED despite its
  successful brightness pair because it retained a stale Tab frame after
  Alt-Tab. The `16 ms` refresh timer is the tested repair.
- Paired 10-second process-tree sampling on the same installed process:
  OFF `1.360` CPU seconds / `604.1 MB` working / `485.2 MB` private; ON `2.373`
  CPU seconds / `618.4 MB` working / `598.0 MB` private. Increment: about
  `10.1%` of one logical core, `14.3 MB` working set, and `112.8 MB` private.

## Pending installed proof

### UNKNOWN

- Final verification commit.
- Matched OFF/ON live night-scene screenshots and luminance metrics.
- Final boost-b Alt-Tab/refocus HWND recreation and graceful exit cleanup.
- User choice: accepted, still too dark, or too bright.

### DECISION

Do not push, tag, or publish v1.7.2 until all installed proof above is recorded
and the user explicitly accepts the live night scene.
