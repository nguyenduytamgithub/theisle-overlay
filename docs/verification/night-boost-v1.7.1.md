# Night Boost v1.7.1 Candidate Verification

**Recorded:** 2026-08-29 09:56:28 +07:00

**Status:** PARTIAL — machine qualification passes; mandatory human dark-scene
acceptance is still UNKNOWN, so this candidate is not released publicly.

## Candidate identity

### FACT

- Git commit: `de73f196b3b5a1725bb4d9abfe1ccc01945aaf20`.
- Version: `1.7.1`.
- Build fingerprint: `1.7.1-visual-boost-a`.
- Release executable:
  `D:\CodexBuild\theisle-overlay-nightboost\release\theisle-overlay.exe`.
- Release executable size: `21,370,880` bytes.
- Release executable SHA-256:
  `3456ED44744EA52C5D1B7AA67EA3FA101909549B9506F4BBBAB3C673C9AFD71D`.
- NSIS installer:
  `D:\CodexBuild\theisle-overlay-nightboost\release\bundle\nsis\TheIsle Overlay_1.7.1_x64-setup.exe`.
- Installer size: `5,806,573` bytes.
- Installer SHA-256:
  `FB88C8A404E77BE5F38C4EC859C4C3A3AE9D08F8F89E60C3EF846224AFEA28D2`.
- Installed executable:
  `C:\Users\Admin\AppData\Local\TheIsle Overlay\theisle-overlay.exe`.
- Installed executable SHA-256:
  `21E791C2A11EAD4598CB18E9F5A6F3EAAE3647BF16CCA3E2648142C13BB9C8DA`.

### EVIDENCE

- The release and installed executables both report product version `1.7.1`
  and contain `night-vision-filter.html` plus fingerprint
  `1.7.1-visual-boost-a`.
- The installed executable differs from the release executable at exactly three
  consecutive bytes, offset `16563226-16563228`: Tauri's bundle marker changes
  from `BUNDLE_TYPE_VAR_UNK` to `BUNDLE_TYPE_VAR_NSS` during NSIS packaging.
  Applying only `NSS` at that offset in memory produces SHA-256
  `21E791C2A11EAD4598CB18E9F5A6F3EAAE3647BF16CCA3E2648142C13BB9C8DA`,
  exactly matching the installed file.

## Automated gates

### FACT

- `cargo fmt --check` on `src-tauri/src/night_vision.rs`: PASS.
- `npm run check`: PASS, 0 errors and 0 warnings.
- `npm run build`: PASS; production output contains
  `night-vision-filter.html`.
- `cargo test --all-targets`: PASS; library 98 passed / 9 ignored / 0 failed,
  safety 4 passed, release configuration 3 passed.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `npm run tauri build`: PASS using
  `CARGO_TARGET_DIR=D:\CodexBuild\theisle-overlay-nightboost`.

## Installed runtime

### FACT

- The candidate was installed silently after stopping only TheIsle Overlay.
- The Isle launcher and `TheIsleClient-Win64-Shipping.exe` remained running.
- With the game foreground and Night Vision requested at strength 70, the
  `night vision filter` window became visible.
- Runtime log acknowledgement:
  `visual boost painted request=1 strength=70 fingerprint=1.7.1-visual-boost-a`.
- Alt-Tab hid the filter and all in-game overlay surfaces.
- Returning to the game recreated the visible surfaces and emitted:
  `visual boost painted request=2 strength=70 fingerprint=1.7.1-visual-boost-a`.
- Switching Night Vision off hid only the filter; minimap, navigation HUD, and
  Night Vision button remained present.
- `night-vision-recovery.json` was absent after switch-off.
- The screen-capture helper could not capture the composed game frame on this
  machine and returned `SetIsBorderRequired failed: No such interface supported
  (0x80004002)`. Window state and painted acknowledgements are therefore machine
  evidence, not visual-effect proof.

### PERFORMANCE EVIDENCE

Paired 10-second samples over the overlay root process and all descendants:

| Mode | CPU, total machine | Working set | Private memory |
|---|---:|---:|---:|
| Night Vision ON | 0.34% | 622.6 MiB | 543.3 MiB |
| Night Vision OFF | 0.21% | 571.7 MiB | 540.7 MiB |
| ON minus OFF | 0.13 percentage points | 50.9 MiB | 2.6 MiB |

The enabled CPU is below 1%. The incremental working-set and private-memory
costs are below the 96 MiB and 32 MiB feature thresholds. The absolute working
set includes the pre-existing map, settings, minimap, HUD, button, and WebView2
processes, so it is recorded but not attributed to Night Vision.

## Decision

### PASS

- Build, tests, package identity, local installation, painted acknowledgement,
  focus lifecycle, switch-off cleanup, and incremental performance.

### UNKNOWN

- Whether the actual current in-game night scene is visibly and usefully
  brighter to the user.
- Whether strength 70 is the user's preferred balance between visibility and a
  pale image.

### DECISION

Do not push, tag, or publish v1.7.1 until the user explicitly confirms the
installed candidate is clearly brighter in a real dark scene. If it is still
too dark or too pale, tune the opacity mapping, rebuild, reinstall, and repeat
the same gates.
