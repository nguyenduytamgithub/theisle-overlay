# Night Boost v1.7.2 Candidate Verification

**Recorded:** 2026-08-29

**Status:** PARTIAL — source behavior, safety gates, and candidate packaging
pass; installed runtime, matched night-scene captures, performance, lifecycle,
and human visual acceptance are pending.

## Candidate design

### FACT

- Version target: `1.7.2`.
- Build fingerprint target: `1.7.2-magnifier-boost-a`.
- The rejected constant-color v1.7.1 WebView paint path has been removed.
- The existing click-through Tauri window hosts a native Windows Magnification
  child that applies an identity spatial transform and diagonal RGB gain.
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

- Full Rust suite: PASS (`102` passed, `0` failed, `9` deliberately ignored
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
- Size: `5,812,365` bytes.
- SHA-256: `94BE10ECDFDAD68E4290678873106ABF712B2B62CDBB4D018BECA8FE449B1FF3`.
- Tauri release build and NSIS packaging: PASS.

## Pending installed proof

### UNKNOWN

- Final commit and installed executable version/hash.
- Installed executable version/hash.
- Matched OFF/ON live night-scene screenshots and luminance metrics.
- Click-through, Alt-Tab, OFF, refocus, HWND recreation, and exit cleanup.
- Paired 10-second CPU, working-set, and private-memory samples.
- User choice: accepted, still too dark, or too bright.

### DECISION

Do not push, tag, or publish v1.7.2 until all installed proof above is recorded
and the user explicitly accepts the live night scene.
