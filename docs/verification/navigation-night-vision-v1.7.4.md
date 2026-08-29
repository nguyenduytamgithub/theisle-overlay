# v1.7.4 Navigation + Magnifier Night Boost verification

Date: 2026-08-29 (Asia/Bangkok)

## Candidate identity

- Branch: `codex/navigation-hud`
- Integration merge: `f608aeb9f78fbda94cdc67d03c7f7518b0342f99`
- Merge parents: `d895066` (Night Vision/API resilience line) and `21ae583`
  (deterministic navigation line)
- Version: `1.7.4` in npm, Cargo, Tauri, PE FileVersion, and PE ProductVersion

This is a candidate record, not a claim that the public release gate has passed.

## Automated verification

- Navigation estimator: 34 passed, 0 failed.
- Svelte diagnostics: 0 errors, 0 warnings.
- Rust workspace:
  - coordinate/calibration/tracker integration: 48 passed, 0 failed;
  - application library: 110 passed, 0 failed, 9 ignored live-data tests;
  - Night Vision safety: 4 passed, 0 failed;
  - release configuration: 4 passed, 0 failed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- Production Vite build: passed.
- `scripts/check-forbidden-apis.ps1`: passed.
- Version consistency check: `1.7.4` everywhere.
- Secret-shape scan: no matching credential-shaped assignment was found.

Ignored Rust tests require downloaded/live external data and are not counted as
runtime acceptance.

## Built artifacts

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| NSIS installer | 5,814,158 | `9F341680E91BF3D20F74DCD9B3F06271D9C6CD63C85DEBB2EB2AE247E714699C` |
| Release executable before NSIS bundle marker patch | 21,459,968 | `F0430DBDD844FC9B9084A7FC100C5AB928877EB03A2D20E886800F23A21421A6` |
| Installed executable | 21,459,968 | `301369E5245AC7959B4BAFCFB0CDEAB24964494FC0098475AE797556324EC793` |

The installed executable differs from the release executable at exactly three
bytes (`00FDE80A` through `00FDE80C`) inside Tauri's
`_TAURI_BUNDLE_TYPE_VAR_*` marker. Tauri logged that it patched this marker for
NSIS before packaging. Replacing only those three marker bytes in memory makes
the release SHA-256 equal the installed SHA-256; no application-code byte differs.
Both files report FileVersion and ProductVersion `1.7.4`. The artifacts are not
code-signed, so SmartScreen may warn.

## Installation and data preservation

- Silent NSIS install exit code: 0.
- Installed path: `C:\Users\Admin\AppData\Local\TheIsle Overlay\theisle-overlay.exe`.
- Installed overlay process was responsive after launch.
- The Isle stayed on the same PID and remained responsive throughout replacement.
- The DPAPI-encrypted IslePilot token hash was unchanged.
- `waypoints.json` hash was unchanged.
- Settings migrated to navigation schema 2 with the default arrival radius 25 m;
  HUD visibility, Night Vision strength 70, and IslePilot poll interval 5 s were
  present after migration.
- Fifteen trail files remained present after installation, and a new session
  trail was created after v1.7.4 launched.

No credential, token value, cookie, or private server URL is included in this
record.

## Current runtime evidence

After the installed v1.7.4 process started, the local log recorded fresh
confirmed server samples and the HUD estimator changing through `tracking`,
`estimating`, and `waiting` states. This proves that the installed navigation
pipeline, HUD webview, IslePilot sample path, and 4-to-12-second freshness state
machine ran together on this machine.

The update endpoint also returned an unsuccessful status during startup. This
fork documents manual updates; that log entry did not stop position updates or
the overlay process, but it is not described as a successful automatic-update
check.

## Acceptance boundary

Automated checks and current runtime telemetry are **PASS** for installation,
data preservation, confirmed-position ingestion, and estimator state changes.

The following remain **live acceptance**, not inferred completion:

1. Select a real waypoint, travel far enough to observe the absolute north-up
   arrow and maneuver text, then confirm arrival latches inside 25 m.
2. During a genuinely dark scene, toggle Night Vision in this exact v1.7.4 build
   and confirm the native Magnification contrast boost is visibly useful.

Until those two observations are recorded, this candidate must not be tagged or
advertised as a fully accepted public release.
