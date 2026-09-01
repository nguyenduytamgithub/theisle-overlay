# Compact Waypoint XY Board Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make waypoint `➤` display the shared north-up XY board over the game, with exclusive freshwater/waypoint sources and a compact presentation.

**Architecture:** Reuse the existing `water-guide` full-game window and pure XY geometry. The backend coordinates the two destination sources and shows the window for either request; the frontend selects the active route, while HUD/minimap suppression follows the combined overlay state.

**Tech Stack:** TypeScript 5, Node test runner, Svelte 5, Vite 7, Tauri 2, Rust 2021, CSS.

## Global Constraints

- `➤` starts on-screen waypoint guidance; `■` stops it.
- `Ctrl+Alt+W` continues to toggle freshwater guidance.
- Selecting a waypoint turns Water Guide off; activating Water Guide clears the waypoint target.
- Both modes use the same target/movement needles and fixed north-up board.
- Mouse, camera, head/facing, game memory, injection/hooks, packets, continuous capture, and synthetic game input are forbidden inputs.
- The board is capped at 230 px, uses one compact status pill, and remains click-through.
- The normal HUD and rotating minimap are hidden while either shared-board source is active.
- Preserve the game; packaging may stop/restart only Overlay.

---

### Task 1: Coordinate destination sources and window visibility

**Files:**
- Modify: `src-tauri/src/water_guide.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/water_guide_window.rs`

**Interfaces:**
- Produces: `water_guide::deactivate_for_waypoint(app: &AppHandle)`.
- Consumes: `commands::apply_settings_patch`, `NAVIGATION_CHANGED`, `WaterGuideRuntime`, and `navigation.target_waypoint_id`.

- [ ] **Step 1: Write failing Rust regressions**

Add combined window-policy assertions:

```rust
assert!(should_show(true, false, true, false));
assert!(should_show(false, true, true, false));
assert!(!should_show(false, false, true, false));
assert!(!should_show(true, true, false, false));
assert!(!should_show(true, true, true, true));
```

Add a `WaterGuideRuntime` test that activates a route, explicitly deactivates it, and requires `requested == false`, `route == None`, and `error_key == None`.

- [ ] **Step 2: Run RED**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib water_guide
```

Expected: compile/assertion failure because combined visibility and explicit waypoint deactivation do not exist.

- [ ] **Step 3: Implement exclusivity and combined request state**

Change the window `Snapshot` to hold `water_requested` and `waypoint_requested`; derive the latter from a non-empty `navigation.target_waypoint_id`. Show the window when either is active, the game is foreground, and the main app is not in front.

Add an idempotent runtime deactivation operation and publish `water-guide://changed` only when Water Guide was active. In `set_navigation_target`, deactivate Water Guide before applying a valid non-null waypoint. Before Water Guide transitions from off to on, apply a null waypoint target and emit `navigation://changed`.

- [ ] **Step 4: Run GREEN**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/water_guide.rs src-tauri/src/commands.rs src-tauri/src/water_guide_window.rs
git commit -m "feat: coordinate waypoint and water guide sources"
```

### Task 2: Reuse the XY renderer for waypoint targets

**Files:**
- Modify: `src/lib/navigation/water-guide.ts`
- Modify: `src/lib/navigation/water-guide.test.mjs`
- Modify: `src/water-guide/main.ts`
- Modify: `src/lib/navigation/water-guide-style.test.mjs`

**Interfaces:**
- Produces: `waypointGuideRoute(target, anchor): WaterGuideRoute` and renderer source `"water" | "waypoint" | null`.
- Consumes: `active_navigation`, `navigation://changed`, `waypoints://changed`, selected waypoint ID, current position, and `waterGuideFrame`.

- [ ] **Step 1: Write failing pure and renderer tests**

Add a pure test using:

```javascript
const route = waypointGuideRoute(
  { id: "home", name: "Nhà", xCm: 12000, yCm: -4000, distanceM: 125 },
  { xCm: 1000, yCm: 2000 },
);
assert.equal(route.label, "Nhà");
assert.deepEqual([route.startXCm, route.startYCm], [1000, 2000]);
assert.deepEqual([route.targetXCm, route.targetYCm], [12000, -4000]);
```

Require renderer source to contain `active_navigation`, both navigation/waypoint listeners, `waypointGuideRoute`, waypoint copy `ĐIỂM:`, and navigation revision guards.

- [ ] **Step 2: Run RED**

```powershell
node --test src/lib/navigation/water-guide.test.mjs
node --test src/lib/navigation/water-guide-style.test.mjs
```

Expected: missing export and missing renderer wiring failures.

- [ ] **Step 3: Implement shared route selection**

Add the pure conversion function. Keep water state separate from `NavigationTarget`, derive waypoint requested state from settings, refresh navigation on existing events, retry resolution on a position event, and lock the waypoint route start until the selected waypoint changes.

Use freshwater when requested, otherwise the selected waypoint. Waypoint waiting copy is `ĐIỂM GHIM · CHỜ VỊ TRÍ`; resolved copy is `ĐIỂM: <name> · <distance>`. Reuse movement threshold, alignment hysteresis, stale freeze, and arrival behavior.

- [ ] **Step 4: Run GREEN**

```powershell
node --test src/lib/navigation/water-guide.test.mjs
node --test src/lib/navigation/water-guide-style.test.mjs
node --test src/lib/navigation/*.test.mjs
npm run check
```

- [ ] **Step 5: Commit**

```powershell
git add src/lib/navigation/water-guide.ts src/lib/navigation/water-guide.test.mjs src/water-guide/main.ts src/lib/navigation/water-guide-style.test.mjs
git commit -m "feat: guide to selected waypoints on the XY board"
```

### Task 3: Suppress duplicate guidance and compact the board

**Files:**
- Modify: `src/hud/main.ts`
- Modify: `src/minimap/main.ts`
- Modify: `src/lib/navigation/water-guide-suppression.test.mjs`
- Modify: `water-guide.html`
- Modify: `src/water-guide/style.css`
- Modify: `src/lib/navigation/water-guide-style.test.mjs`

**Interfaces:**
- Consumes: Water Guide requested state and selected waypoint ID.
- Produces: one combined board-request decision in HUD/minimap and one compact status pill.

- [ ] **Step 1: Write failing layout and suppression tests**

Require HUD and minimap to combine Water Guide and waypoint state before rendering. Require CSS to cap the disc at `230px`, retain `pointer-events: none`, and remove the large duplicate maneuver banner.

- [ ] **Step 2: Run RED**

```powershell
node --test src/lib/navigation/water-guide-suppression.test.mjs
node --test src/lib/navigation/water-guide-style.test.mjs
```

- [ ] **Step 3: Implement compact shared presentation**

Track water and waypoint requests separately in HUD/minimap, hiding ordinary guidance when either is active. Replace the second maneuver element with one status pill. Use `clamp(190px, 19vw, 230px)`, translucent background, reduced padding, and no animation/transform transition.

- [ ] **Step 4: Run GREEN and build**

```powershell
node --test src/lib/navigation/*.test.mjs
npm run check
npm run build
git diff --check
```

- [ ] **Step 5: Commit**

```powershell
git add src/hud/main.ts src/minimap/main.ts src/lib/navigation/water-guide-suppression.test.mjs water-guide.html src/water-guide/style.css src/lib/navigation/water-guide-style.test.mjs
git commit -m "feat: compact shared XY guidance overlay"
```

### Task 4: Review, package, install, and hand off

**Files:**
- Modify: `docs/verification/water-guide-ray-live.md`
- Modify: `docs/superpowers/plans/2026-09-01-waypoint-xy-board.md`

**Interfaces:**
- Consumes: reviewed clean feature branch and installed Overlay.
- Produces: installer/hash, rollback backup, live evidence when the user supplies a game session, and public Git commit.

- [ ] **Step 1: Synchronize CodeGraph and review**

Run the ensure script and one focused two-file query. Require no camera/input/game-process source and no unresolved review finding.

- [ ] **Step 2: Run final matrix**

```powershell
node --test src/lib/navigation/*.test.mjs
npm run check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
git diff --check
```

- [ ] **Step 3: Build and install only Overlay**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-waypoint-board-target'
npm run tauri -- build --bundles nsis
```

Record hashes, back up the installed EXE to a new explicit `D:\CodexBuild` directory, and stop/restart only `theisle-overlay.exe`.

- [ ] **Step 4: Capture acceptance without controlling the game**

When a game session is available, require `➤` to show the compact board with waypoint name, `■` to hide it, and `Ctrl+Alt+W` to switch exclusively to water. Do not send game input. If unavailable, record automated/runtime evidence honestly and leave manual visual acceptance open.

- [ ] **Step 5: Finalize docs and Git**

Record results, mark completed plan steps, commit docs, verify worktrees are clean, fast-forward the public branch only if it remains an ancestor, and push without force.
