# Newbie Navigation v1.7 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the spinning, server-step-driven guidance with a deterministic local estimator and a stable north-up newbie HUD that remains useful across sparse updates without inventing unbounded realtime position.

**Architecture:** A pure TypeScript `NavigationEstimator` consumes immutable Rust confirmation events and produces a presentation snapshot for any timestamp. Rust remains the confirmed truth boundary and publishes separate server-facing, motion-course, velocity, freshness, and relocation metadata. HUD, minimap, and full map instantiate the same estimator and render equivalent local snapshots; only confirmed Rust trail points are persisted.

**Tech Stack:** TypeScript ES modules, Node test runner, Svelte 5, Leaflet, HTML canvas, Rust, Tauri 2, Cargo.

## Global Constraints

- Version is `1.7.0 Newbie Navigation`.
- Primary destination arrow is absolute north-up and cannot consume raw server yaw.
- UI animation runs locally at approximately 30 FPS; IslePilot polling remains five seconds.
- Projection is constant velocity for four seconds, decays through twelve seconds with a three-second time constant, then holds.
- No game memory, injection, input hooks, synthetic input, packet capture, or continuous screen capture.
- Predicted points are presentation-only and never persisted to confirmed trail files.
- Direct-to-waypoint guidance must not claim terrain-safe pathfinding.
- Installation must preserve the encrypted token, settings, waypoints, and confirmed trails.
- GitHub release must wait for automated gates, local install verification, and an honestly recorded live-game result.

---

### Task 1: Pure angular and projection primitives

**Files:**
- Create: `src/lib/navigation/estimator.ts`
- Create: `src/lib/navigation/estimator.test.mjs`

**Interfaces:**
- Produces: `shortestDeltaDeg(fromDeg, toDeg): number`
- Produces: `advanceAngleDeg(currentDeg, targetDeg, elapsedS, maxRateDegS, deadbandDeg): number`
- Produces: `effectiveProjectionAgeS(ageS, linearHorizonS, holdAfterS, decayTauS): number`
- Produces: `freshnessForAge(ageS): "tracking" | "estimating" | "waiting"`

- [ ] **Step 1: Write the failing primitive tests**

```js
import {
  advanceAngleDeg,
  effectiveProjectionAgeS,
  freshnessForAge,
  shortestDeltaDeg,
} from "./estimator.ts";

test("359 to 1 uses the two-degree short arc", () => {
  assert.equal(shortestDeltaDeg(359, 1), 2);
  assert.equal(shortestDeltaDeg(1, 359), -2);
});

test("visual angle obeys rate limit and crosses north without spinning", () => {
  assert.equal(advanceAngleDeg(350, 10, 0.05, 120, 4), 356);
  assert.equal(advanceAngleDeg(359, 1, 1, 120, 4), 359);
});

test("projection decays and is fully held after twelve seconds", () => {
  assert.equal(effectiveProjectionAgeS(4, 4, 12, 3), 4);
  assert.ok(Math.abs(effectiveProjectionAgeS(16, 4, 12, 3) - 6.79155) < 0.0001);
});

test("freshness labels are honest at boundaries", () => {
  assert.equal(freshnessForAge(6), "tracking");
  assert.equal(freshnessForAge(12), "estimating");
  assert.equal(freshnessForAge(12.001), "waiting");
});
```

- [ ] **Step 2: Run the primitive test and verify RED**

Run: `node --test src/lib/navigation/estimator.test.mjs`  
Expected: FAIL because `estimator.ts` or its exports do not exist.

- [ ] **Step 3: Implement the minimal circular and decay math**

```ts
export const normalizeDeg = (value: number) => ((value % 360) + 360) % 360;

export function shortestDeltaDeg(fromDeg: number, toDeg: number): number {
  return ((toDeg - fromDeg + 540) % 360) - 180;
}

export function advanceAngleDeg(
  currentDeg: number,
  targetDeg: number,
  elapsedS: number,
  maxRateDegS = 120,
  deadbandDeg = 4,
): number {
  const delta = shortestDeltaDeg(currentDeg, targetDeg);
  if (Math.abs(delta) <= deadbandDeg) return normalizeDeg(currentDeg);
  const step = Math.sign(delta) * Math.min(Math.abs(delta), maxRateDegS * Math.max(0, elapsedS));
  return normalizeDeg(currentDeg + step);
}

export function effectiveProjectionAgeS(
  ageS: number,
  linearHorizonS = 4,
  holdAfterS = 12,
  decayTauS = 3,
): number {
  const boundedAge = Math.max(0, Math.min(ageS, holdAfterS));
  if (boundedAge <= linearHorizonS) return boundedAge;
  return linearHorizonS
    + decayTauS * (1 - Math.exp(-(boundedAge - linearHorizonS) / decayTauS));
}

export const freshnessForAge = (ageS: number) =>
  ageS <= 6 ? "tracking" : ageS <= 12 ? "estimating" : "waiting";
```

- [ ] **Step 4: Run estimator primitives and the existing navigation suite**

Run: `node --test src/lib/navigation/estimator.test.mjs src/lib/navigation/prediction.test.mjs src/lib/navigation/guidance.test.mjs`  
Expected: PASS with zero failures.

- [ ] **Step 5: Commit the primitives**

```powershell
git add src/lib/navigation/estimator.ts src/lib/navigation/estimator.test.mjs
git commit -m "feat: add stable navigation math"
```

### Task 2: Stateful local estimator and newbie maneuver model

**Files:**
- Modify: `src/lib/navigation/estimator.ts`
- Modify: `src/lib/navigation/estimator.test.mjs`

**Interfaces:**
- Consumes:

```ts
export interface ConfirmedNavigationSample {
  xCm: number;
  yCm: number;
  px: number;
  py: number;
  velocityXCmS: number | null;
  velocityYCmS: number | null;
  velocityPxXS: number | null;
  velocityPxYS: number | null;
  serverFacingDeg: number | null;
  motionCourseDeg: number | null;
  confirmedAtMs: number;
  relocated: boolean;
}
```

- Produces:

```ts
export interface EstimatorTarget {
  id: string;
  name: string;
  xCm: number;
  yCm: number;
}

export interface NavigationSnapshot {
  xCm: number;
  yCm: number;
  px: number;
  py: number;
  targetBearingDeg: number | null;
  guidanceCourseDeg: number | null;
  targetDistanceM: number | null;
  maneuver: "straight" | "slight-left" | "slight-right" | "left" | "right" | "turn-back" | "hold-cardinal" | "arrived";
  freshness: "tracking" | "estimating" | "waiting";
  predicting: boolean;
  arrived: boolean;
  noProgress: boolean;
}

export class NavigationEstimator {
  accept(sample: ConfirmedNavigationSample): void;
  setTarget(target: EstimatorTarget | null): void;
  snapshot(nowMs: number): NavigationSnapshot | null;
}
```

- [ ] **Step 1: Add failing behavioral tests**

Add literal tests for:

```js
test("noisy server facing cannot rotate the absolute target arrow", () => {
  const nav = estimatorWithEastTarget();
  nav.accept(sample({ serverFacingDeg: 10, confirmedAtMs: 0 }));
  const first = nav.snapshot(0);
  nav.accept(sample({ serverFacingDeg: 280, confirmedAtMs: 5_000 }));
  const second = nav.snapshot(5_000);
  assert.equal(first.targetBearingDeg, 90);
  assert.equal(second.targetBearingDeg, 90);
});

test("sixteen-second gap decays and holds instead of freezing then jumping", () => {
  const nav = estimatorWithVelocity(100);
  assert.equal(Math.round(nav.snapshot(4_000).xCm), 400);
  assert.equal(Math.round(nav.snapshot(12_000).xCm), 679);
  assert.equal(Math.round(nav.snapshot(16_000).xCm), 679);
});

test("arrival freezes guidance inside twenty-five metres", () => {
  const nav = estimatorWithTargetAt(2_400, 0);
  nav.accept(sample({ xCm: 0, yCm: 0 }));
  const view = nav.snapshot(0);
  assert.equal(view.arrived, true);
  assert.equal(view.maneuver, "arrived");
});

test("three confirmations with under ten metres progress warn once", () => {
  const nav = estimatorWithTargetAt(100_000, 0);
  acceptAtDistances(nav, [1000, 998, 995]);
  assert.equal(nav.snapshot(30_000).noProgress, true);
});
```

Also cover source stability for one second, four-degree angular deadband, 600 ms maneuver hysteresis, relocation reset, ordinary 650 ms correction, 30–100 m 300 ms correction, and above-100 m snap.

- [ ] **Step 2: Run the stateful tests and verify RED**

Run: `node --test src/lib/navigation/estimator.test.mjs`  
Expected: FAIL on missing `NavigationEstimator` behavior, not syntax.

- [ ] **Step 3: Implement the estimator state machine**

Use:

```ts
const ARRIVAL_RADIUS_M = 25;
const COURSE_SOURCE_STABLE_MS = 1_000;
const MANEUVER_STABLE_MS = 600;
const NO_PROGRESS_WINDOW = 3;
const NO_PROGRESS_METRES = 10;
```

The target arrow consumes only `bearingTo(projectedPosition, target)`. Guidance course chooses a nonzero-velocity motion course first, then a stable filtered server facing, otherwise unknown. Maintain confirmed-distance history only when `accept()` receives a new confirmation.

- [ ] **Step 4: Run all navigation tests**

Run: `node --test src/lib/navigation/*.test.mjs`  
Expected: PASS with zero failures.

- [ ] **Step 5: Commit the estimator**

```powershell
git add src/lib/navigation/estimator.ts src/lib/navigation/estimator.test.mjs
git commit -m "feat: add confidence-aware navigation estimator"
```

### Task 3: Rust truth metadata and schema-v2 arrival migration

**Files:**
- Modify: `src-tauri/crates/overlay-core/src/tracker.rs`
- Modify: `src-tauri/crates/overlay-core/tests/tracker.rs`
- Modify: `src-tauri/src/events.rs`
- Modify: `src-tauri/src/pipeline.rs`
- Modify: `src-tauri/src/settings.rs`
- Modify: `src/lib/api.ts`

**Interfaces:**
- Produces: `PositionTracker::server_facing(now_s) -> Option<f64>`
- Produces: `PositionTracker::motion_course(now_s) -> Option<f64>`
- Extends `PositionUpdate` with camelCase fields:

```rust
pub server_facing_deg: Option<f64>,
pub motion_course_deg: Option<f64>,
pub relocated: bool,
pub refreshed_only: bool,
```

- [ ] **Step 1: Write failing Rust tracker and settings tests**

```rust
#[test]
fn motion_course_is_independent_from_server_facing() {
    let mut t = tracker();
    t.add_sample_with_heading(0.0, 0.0, 0.0, Some(270.0), 0.0);
    t.add_sample_with_heading(0.0, 10_000.0, 0.0, Some(5.0), 10.0);
    assert_eq!(t.server_facing(10.0), Some(5.0));
    assert_eq!(t.motion_course(10.0), Some(90.0));
}

#[test]
fn schema_v1_default_arrival_migrates_but_custom_survives() {
    let legacy = json!({"navigation":{"schema_version":1,"arrival_radius_m":15.0}});
    assert_eq!(merge_loaded_settings(&legacy)["navigation"]["arrival_radius_m"], 25.0);
    let custom = json!({"navigation":{"schema_version":1,"arrival_radius_m":40.0}});
    assert_eq!(merge_loaded_settings(&custom)["navigation"]["arrival_radius_m"], 40.0);
}
```

Add a pipeline payload test proving relocation and refreshed-only flags reach the serialized event.

- [ ] **Step 2: Run targeted Rust tests and verify RED**

Run:

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-newbie-navigation'
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml motion_course_is_independent_from_server_facing
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml schema_v1_default_arrival_migrates_but_custom_survives
```

Expected: FAIL because the methods/fields/schema-v2 migration do not exist.

- [ ] **Step 3: Split facing and course, extend the payload, migrate settings**

Refactor the current heading fallback:

```rust
pub fn server_facing(&self, now_s: f64) -> Option<f64> {
    let current = self.current?;
    (current.age_s(now_s) <= HEADING_MAX_AGE_S)
        .then_some(current.heading_deg)
        .flatten()
}

pub fn motion_course(&self, now_s: f64) -> Option<f64> {
    let current = self.current?;
    if current.age_s(now_s) > HEADING_MAX_AGE_S { return None; }
    let anchor = self.history.iter().find(|sample| {
        current.at_s - sample.at_s <= HEADING_MAX_AGE_S
            && distance_m(sample.x, sample.y, current.x, current.y) >= HEADING_MIN_DISTANCE_M
    })?;
    Some(bearing_deg(anchor.x, anchor.y, current.x, current.y, &self.cal))
}
```

Keep legacy `heading_deg` and `heading_source` for compatibility, but populate the new independent fields. Set navigation defaults to schema 2 and 25 metres. Migrate schema-1 value 15 to 25 and preserve any other numeric value.

- [ ] **Step 4: Run Rust tracker, pipeline, and settings suites**

Run:

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-newbie-navigation'
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --workspace --manifest-path src-tauri\Cargo.toml
```

Expected: all non-live tests PASS; only existing explicitly ignored live/fixture tests remain ignored.

- [ ] **Step 5: Commit the truth metadata**

```powershell
git add src-tauri/crates/overlay-core/src/tracker.rs src-tauri/crates/overlay-core/tests/tracker.rs src-tauri/src/events.rs src-tauri/src/pipeline.rs src-tauri/src/settings.rs src/lib/api.ts
git commit -m "feat: expose navigation quality metadata"
```

### Task 4: Replace HUD with stable newbie guidance

**Files:**
- Modify: `hud.html`
- Modify: `src/hud/main.ts`
- Modify: `src/hud/style.css`
- Modify: `src/lib/navigation/estimator.test.mjs`

**Interfaces:**
- Consumes: `NavigationEstimator.accept`, `setTarget`, and `snapshot`
- Produces DOM fields: `target-arrow`, `instruction`, `target-detail`, `course`, `freshness`, `progress-warning`

- [ ] **Step 1: Add failing presentation-copy tests**

```js
test("Vietnamese newbie copy is explicit and nontechnical", () => {
  assert.equal(localizeManeuver("slight-left", "vi"), "CHẾCH TRÁI");
  assert.equal(localizeManeuver("hold-cardinal", "vi", "ĐÔNG BẮC"), "GIỮ HƯỚNG ĐÔNG BẮC");
  assert.equal(localizeFreshness("estimating", "vi"), "ĐANG ƯỚC LƯỢNG");
  assert.equal(localizeFreshness("waiting", "vi"), "CHỜ SERVER");
});
```

- [ ] **Step 2: Run the copy test and verify RED**

Run: `node --test src/lib/navigation/estimator.test.mjs`  
Expected: FAIL on missing localization exports.

- [ ] **Step 3: Wire HUD to one 30 FPS estimator loop**

Remove inline `projectedPosition`, `smoothedPosition`, raw `relativeBearing`, and old technical labels from `src/hud/main.ts`. On each `position://update`, call `estimator.accept(payload)`. On target changes, call `estimator.setTarget(target)`. Render:

```ts
const view = estimator.snapshot(nowMs);
targetArrowEl.style.transform = `rotate(${view.targetBearingDeg ?? 0}deg)`;
instructionEl.textContent = localizeManeuver(view.maneuver, language, targetCardinal);
freshnessEl.textContent = localizeFreshness(view.freshness, language);
```

Use the large arrow only for absolute target bearing. Show the compact guidance course separately. Freeze the arrival arrow and show `ĐÃ TỚI KHU VỰC ĐÍCH`. Show `ĐANG ĐI XA ĐÍCH — KIỂM TRA BẢN ĐỒ` only when `noProgress` is true.

- [ ] **Step 4: Run navigation tests, Svelte check, and frontend build**

Run:

```powershell
node --test src\lib\navigation\*.test.mjs
npm run check
npm run build
```

Expected: tests PASS, Svelte reports 0 errors/0 warnings, Vite build exits 0.

- [ ] **Step 5: Commit the newbie HUD**

```powershell
git add hud.html src/hud/main.ts src/hud/style.css src/lib/navigation/estimator.test.mjs
git commit -m "feat: replace spinning arrow with newbie guidance"
```

### Task 5: Give minimap and full map equivalent local guidance

**Files:**
- Modify: `src/minimap/main.ts`
- Modify: `src/minimap/render.ts`
- Modify: `src/main/fullmap/FullMap.svelte`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/navigation/estimator.test.mjs`

**Interfaces:**
- Each webview creates its own `NavigationEstimator`.
- `MinimapState.predictionTailPx` is `[[number, number], [number, number]] | null`.
- Full map stores `confirmedPosition` and estimator snapshot separately.

- [ ] **Step 1: Add a failing deterministic-equivalence test**

```js
test("independent consumers return equivalent snapshots for one event and timestamp", () => {
  const a = configuredEstimator();
  const b = configuredEstimator();
  const p = sample({ confirmedAtMs: 1_000 });
  a.accept(p);
  b.accept(p);
  assert.deepEqual(a.snapshot(7_500), b.snapshot(7_500));
});
```

- [ ] **Step 2: Run the equivalence test and verify RED**

Run: `node --test src/lib/navigation/estimator.test.mjs`  
Expected: FAIL until deterministic state/reset behavior is complete.

- [ ] **Step 3: Replace legacy prediction in both maps**

In minimap, update `state.position` from the snapshot and draw `predictionTailPx` as a short dashed segment from the last confirmed pixel to the local pixel. The selected-target rim arrow uses the local snapshot bearing, not the backend's stale `bearingDeg`.

In full map, update the player marker, follow camera, direct target line, and midpoint arrow from the same local snapshot. Keep confirmed trail polylines solid. Do not append the local position to `TrailPayload`.

- [ ] **Step 4: Run frontend and Rust persistence gates**

Run:

```powershell
node --test src\lib\navigation\*.test.mjs
npm run check
npm run build
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-newbie-navigation'
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml accepted_duplicate_updates_heading_but_not_the_trail_file
```

Expected: all commands exit 0 and the persistence regression remains green.

- [ ] **Step 5: Commit map integration**

```powershell
git add src/minimap/main.ts src/minimap/render.ts src/main/fullmap/FullMap.svelte src/lib/api.ts src/lib/navigation/estimator.test.mjs
git commit -m "feat: share newbie guidance across maps"
```

### Task 6: Bound HUD readiness recovery and add privacy-safe diagnostics

**Files:**
- Modify: `src-tauri/src/hud.rs`
- Modify: `src-tauri/src/pipeline.rs`
- Modify: `src/hud/main.ts`

**Interfaces:**
- Produces a pure `ReadyRecovery` state with at most one pre-supervisor recreation.
- Logs source, age, correction magnitude, state transitions, and reset reason only.

- [ ] **Step 1: Write failing bounded-recovery tests**

```rust
#[test]
fn missing_ready_allows_one_recreation_then_starts_supervisor() {
    let mut recovery = ReadyRecovery::default();
    assert_eq!(recovery.on_timeout(), ReadyAction::Recreate);
    assert_eq!(recovery.on_timeout(), ReadyAction::StartSupervisor);
    assert_eq!(recovery.on_timeout(), ReadyAction::None);
}
```

- [ ] **Step 2: Run the HUD test and verify RED**

Run:

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-newbie-navigation'
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --manifest-path src-tauri\Cargo.toml missing_ready_allows_one_recreation_then_starts_supervisor
```

Expected: FAIL because `ReadyRecovery` does not exist.

- [ ] **Step 3: Implement one bounded recreation and transition-only diagnostics**

The timeout path closes/recreates the HUD once, waits for the new ready event, then starts the supervisor exactly once. It never loops window recreation. Diagnostics exclude token, cookie, URL, account, and raw credential values.

- [ ] **Step 4: Run Rust tests and Clippy**

Run:

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-newbie-navigation'
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --workspace --manifest-path src-tauri\Cargo.toml
& 'C:\Users\Admin\.cargo\bin\cargo.exe' clippy --workspace --all-targets --manifest-path src-tauri\Cargo.toml -- -D warnings
```

Expected: all tests pass and Clippy exits 0.

- [ ] **Step 5: Commit recovery**

```powershell
git add src-tauri/src/hud.rs src-tauri/src/pipeline.rs src/hud/main.ts
git commit -m "fix: bound HUD recovery and navigation diagnostics"
```

### Task 7: Version, documentation, release build, safe install, and acceptance

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `README.md`
- Modify: `README.en.md`
- Create: `docs/releases/v1.7.0-newbie-navigation.md`

**Interfaces:**
- Produces: `TheIsle Overlay_1.7.0_x64-setup.exe`
- Preserves: `%LOCALAPPDATA%\TheIsleOverlay\islepilot_token.bin`
- Preserves: `%APPDATA%\TheIsleOverlay\settings.json`, `waypoints.json`, and `trails\`

- [ ] **Step 1: Update version and user-facing documentation**

Document:

- stable north-up destination arrow;
- plain Vietnamese maneuvers;
- 30 FPS local presentation versus sparse confirmed server truth;
- tracking/estimating/waiting states;
- direct-line terrain limitation;
- 25 m arrival behavior;
- recovery hotkeys.

- [ ] **Step 2: Run complete verification before packaging**

Run:

```powershell
node --test src\lib\navigation\*.test.mjs
npm run check
npm run build
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-newbie-navigation'
& 'C:\Users\Admin\.cargo\bin\cargo.exe' test --workspace --manifest-path src-tauri\Cargo.toml
& 'C:\Users\Admin\.cargo\bin\cargo.exe' clippy --workspace --all-targets --manifest-path src-tauri\Cargo.toml -- -D warnings
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-forbidden-apis.ps1
git diff --check
```

Expected: every command exits 0; ignored live tests are listed honestly.

- [ ] **Step 3: Build the NSIS installer**

Run:

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-newbie-navigation'
npm run tauri build
```

Expected: Tauri produces the 1.7.0 NSIS installer under the D: target directory without requiring an updater signing key.

- [ ] **Step 4: Verify preservation targets, then install over v1.6.0**

Record pre-install hashes/metadata for settings, waypoints, trails manifest, and encrypted token without printing their contents. Stop only `theisle-overlay.exe`, never The Isle. Run the installer silently if supported; otherwise open it for the user. Relaunch overlay and verify version 1.7.0, responsive process, HUD ready, exact preservation of token/waypoints/trails, and a settings diff limited to the documented schema-2/default-arrival migration.

- [ ] **Step 5: Run replay and live acceptance**

Replay synthetic five/16-second samples and verify no full spin, bounded correction, freshness transitions, and decreasing distance. For live acceptance, ask the user only to walk toward one selected waypoint across at least three confirmed updates while logs collect privacy-safe navigation transitions.

- [ ] **Step 6: Commit release metadata**

```powershell
git add package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json README.md README.en.md docs/releases/v1.7.0-newbie-navigation.md
git commit -m "release: prepare newbie navigation v1.7.0"
```

- [ ] **Step 7: Push and publish only after live result**

Push `navigation-hud` without force, create tag `v1.7.0-newbie-navigation`, upload the verified installer, and read back public repo/default branch/release/asset size/digest. If live acceptance exposes a defect, do not publish; return to the smallest failing regression test.
