# Reliable Navigation and In-Game Compass Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a custom TheIsle Overlay build that rejects impossible position spikes, uses authoritative yaw, predicts motion honestly for at most four seconds, navigates to a selected waypoint, and displays a click-through Vietnamese compass HUD over the game.

**Architecture:** Pure Rust tracker logic remains the source of truth for accepted samples, heading, velocity, and trail segmentation. Tauri events expose confirmed motion state and a persisted active waypoint; shared pure TypeScript computes visual prediction and relative turn for the minimap, full map, and a new lightweight HUD webview.

**Tech Stack:** Rust 2021, Tauri 2, Svelte 5, TypeScript 5, Vite 7, Vitest, Leaflet, Windows Win32 window anchoring.

## Global Constraints

- Never read game memory, inject into The Isle, synthesize game input, or modify anti-cheat/game files.
- Never log or expose IslePilot cookies, tokens, or raw authenticated responses.
- Predicted positions are visual-only, never written to trail storage.
- Prediction stops after 4 seconds; stale state begins after 12 seconds.
- The active waypoint is straight-line navigation, not terrain-aware routing.
- Preserve existing settings, waypoint, trail, clipboard, and basemap compatibility.
- Do not control the user's screen or interrupt the running game until final in-game QA is genuinely required.

---

### Task 1: Deterministic tracker acceptance and motion state

**Files:**
- Modify: `src-tauri/crates/overlay-core/src/tracker.rs`
- Modify: `src-tauri/crates/overlay-core/tests/tracker.rs`
- Modify: `src-tauri/crates/overlay-core/src/coords.rs`
- Modify: `src-tauri/crates/overlay-core/src/lib.rs`

**Interfaces:**
- Consumes: calibrated game-centimetre coordinates and deterministic `now_s`.
- Produces: `add_sample_with_heading(x, y, z, heading_deg, now_s) -> SampleOutcome`, `motion() -> MotionEstimate`, and `game_yaw_to_bearing(yaw) -> f64`.

- [ ] **Step 1: Write failing spike and relocation tests**

Add tests that assert a 7,931.6 m/3 s candidate returns `accepted == false`, leaves `current` and `segments` unchanged, and a return near the confirmed location resumes normally. Add a second test where the next sample is near the quarantined point and assert `relocated == true`, a new segment begins, and velocity is absent.

```rust
let before = t.current.unwrap();
let out = t.add_sample_with_heading(752_257.0, -240.0, 0.0, None, 3.0);
assert!(!out.accepted);
assert_eq!(t.current, Some(before));
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run: `cargo test -p overlay-core --test tracker impossible_spike -- --nocapture`

Expected: compile failure because `add_sample_with_heading` and `SampleOutcome::accepted` do not exist.

- [ ] **Step 3: Implement quarantine and confirmed relocation minimally**

Add finite checks, elapsed-time speed allowance, `pending_jump`, and explicit outcome fields. A rejected sample must return before rotating `previous/current` or appending trail nodes. A confirmed relocation must clear derived velocity and begin a new segment.

- [ ] **Step 4: Run tracker tests and verify GREEN**

Run: `cargo test -p overlay-core --test tracker`

Expected: all tracker tests pass.

- [ ] **Step 5: Write failing tests for delayed plausible movement, authoritative heading, and yaw conversion**

Cover a 324 m/34 s move remaining in one segment, server heading winning over noisy movement, stale server heading falling back to recent accepted motion, and:

```rust
assert_eq!(game_yaw_to_bearing(0.0), 180.0);
assert_eq!(game_yaw_to_bearing(90.0), 90.0);
assert_eq!(game_yaw_to_bearing(-90.0), 270.0);
```

- [ ] **Step 6: Run the new tests and verify RED**

Run: `cargo test -p overlay-core --test tracker heading_`

Expected: failures because heading precedence/history and yaw conversion are absent.

- [ ] **Step 7: Implement rolling accepted history, heading precedence, and velocity**

Store a bounded recent accepted history, authoritative heading with timestamp, and velocity components. Normal segment breaks use elapsed-time plausibility plus configured time gap; rejected samples and relocations never create headings.

- [ ] **Step 8: Run all overlay-core tests and commit**

Run: `cargo test -p overlay-core`

Commit: `git commit -am "fix: reject impossible navigation samples"`

---

### Task 2: Preserve IslePilot yaw and emit confirmed motion metadata

**Files:**
- Modify: `src-tauri/src/islepilot/api.rs`
- Modify: `src-tauri/src/islepilot/mod.rs`
- Modify: `src-tauri/src/islepilot/parser.rs`
- Modify: `src-tauri/src/pipeline.rs`
- Modify: `src-tauri/src/events.rs`
- Modify: `src/lib/api.ts`

**Interfaces:**
- Consumes: marker JSON `x/y/yaw`, token `OverlayPosition`, HTML `MapPosition.heading_deg`.
- Produces: `PositionUpdate` with confirmed coordinate, velocity in cm/s and px/s, `confirmedAtMs`, `headingSource`, and `predictionHorizonS`.

- [ ] **Step 1: Write failing parser/adapter tests**

Change the existing marker fixture assertion to require a parsed structure containing swapped coordinates and converted bearing. Extend token API tests to assert the position adapter returns `z` and converted yaw.

- [ ] **Step 2: Run IslePilot tests and verify RED**

Run: `cargo test --lib islepilot::`

Expected: type/assertion failures because adapters currently return only `(x, y)`.

- [ ] **Step 3: Implement a typed `PositionSample` adapter**

Use one internal value carrying `x_cm`, `y_cm`, `z_cm`, and optional compass `heading_deg`. Convert raw Unreal yaw only at the API/marker boundary; pass HTML map rotation unchanged.

- [ ] **Step 4: Write a failing pipeline test for rejected samples**

Extract a pure payload builder where practical and assert a rejected tracker outcome produces no persisted node and no position payload.

- [ ] **Step 5: Implement optional heading ingestion and enriched payloads**

Change the shared pipeline signature to accept optional bearing. Only accepted/refreshed samples emit position; only accepted trail changes persist. Derive pixel velocity with the active calibration.

- [ ] **Step 6: Run Rust tests and frontend type check**

Run: `cargo test --workspace`

Run: `npm run check`

- [ ] **Step 7: Commit**

Commit: `git commit -am "feat: preserve server heading in position pipeline"`

---

### Task 3: Persist and compute the selected navigation target

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/api.ts`
- Modify: `src/main/fullmap/LayerPanel.svelte`
- Modify: `src/main/fullmap/FullMap.svelte`
- Modify: `src/lib/i18n/vi.ts`
- Modify: `src/lib/i18n/en.ts`

**Interfaces:**
- Produces: `set_navigation_target(id: Option<String>)`, `active_navigation() -> Option<NavigationTarget>`, and `navigation://changed`.

- [ ] **Step 1: Write failing Rust tests for target selection and deletion**

Test target validation, bearing/distance from the latest confirmed position, 15 m arrival, persistence patch shape, and target clearing when its waypoint is deleted.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test --lib navigation_target`

Expected: missing command/state types.

- [ ] **Step 3: Implement backend navigation target commands**

Store `navigation.target_waypoint_id` in settings. Never silently substitute the nearest waypoint. Emit `navigation://changed` after selection, waypoint rename/color change, deletion, and position update.

- [ ] **Step 4: Add list selection UI and full-map route layer**

Give every waypoint a `Dẫn đường`/`Dừng` action and active styling. Draw one non-interactive blue line from confirmed position to the active target and update only on accepted position/target events.

- [ ] **Step 5: Run checks and commit**

Run: `cargo test --workspace`

Run: `npm run check`

Commit: `git commit -am "feat: navigate to a selected waypoint"`

---

### Task 4: Shared visual prediction and minimap integration

**Files:**
- Create: `src/lib/navigation.ts`
- Create: `src/lib/navigation.test.ts`
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src/minimap/main.ts`
- Modify: `src/minimap/render.ts`
- Modify: `src/main/fullmap/FullMap.svelte`

**Interfaces:**
- Produces: `predictPosition(update, elapsedS)`, `shortestAngleDelta(from, to)`, and `relativeTurn(heading, targetBearing)`.

- [ ] **Step 1: Install Vitest test infrastructure**

Run: `npm install --save-dev vitest`

Add script: `"test": "vitest run"`.

- [ ] **Step 2: Write failing pure navigation tests**

Assert prediction advances by velocity for 0-4 s, clamps at 4 s, freezes without velocity, marks stale at 12 s, wraps 350° to 10° through +20°, computes left/right/straight, and identifies arrival within 15 m.

- [ ] **Step 3: Run tests and verify RED**

Run: `npm test -- src/lib/navigation.test.ts`

Expected: module/functions missing.

- [ ] **Step 4: Implement minimal pure functions and verify GREEN**

Run: `npm test -- src/lib/navigation.test.ts`

- [ ] **Step 5: Integrate requestAnimationFrame rendering**

The minimap draws confirmed trail solid and current predicted extension dashed. Full map animates only while visible; hidden tabs park the newest confirmed state and do no animation work. Use shortest-angle rotation for player arrows.

- [ ] **Step 6: Run tests/check/build and commit**

Run: `npm test`

Run: `npm run check`

Run: `npm run build`

Commit: `git add package.json package-lock.json src/lib/navigation.ts src/lib/navigation.test.ts src/minimap src/main/fullmap && git commit -m "feat: add bounded visual position prediction"`

---

### Task 5: In-game Vietnamese compass HUD

**Files:**
- Create: `hud.html`
- Create: `src/hud/main.ts`
- Create: `src/hud/render.ts`
- Create: `src/hud/render.test.ts`
- Modify: `vite.config.ts`
- Modify: `src/lib/errlog.ts` only if a new label type is required

**Interfaces:**
- Consumes: `position://update`, `navigation://changed`, settings changes.
- Produces: `hud://ready`; a 220 x 220 transparent canvas with north-up labels and two arrows.

- [ ] **Step 1: Write failing render-model tests**

Test Vietnamese cardinal labels, heading text, target turn/distance, confirmed/predicted/stale status, and absence of arrows when heading/target are unknown.

- [ ] **Step 2: Run the HUD tests and verify RED**

Run: `npm test -- src/hud/render.test.ts`

- [ ] **Step 3: Implement the minimal canvas renderer**

Always render `BẮC`, `ĐÔNG`, `NAM`, `TÂY`; yellow absolute heading arrow; blue absolute target arrow; and status text. Keep the page dependency-free and click-free.

- [ ] **Step 4: Add the Vite entry and event loop**

Add `hud: fileURLToPath(new URL("./hud.html", import.meta.url))`. Start animation only while prediction is active; otherwise redraw on events.

- [ ] **Step 5: Run frontend verification and commit**

Run: `npm test && npm run check && npm run build`

Commit: `git add hud.html vite.config.ts src/hud && git commit -m "feat: add Vietnamese navigation HUD"`

---

### Task 6: Safe Tauri HUD window, visibility, and hotkey

**Files:**
- Create: `src-tauri/src/navigation_hud.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/hotkeys.rs`
- Modify: `src-tauri/src/settings.rs`
- Modify: `src/main/settings/Settings.svelte`
- Modify: `src/main/guide/Guide.svelte`
- Modify: `src/lib/i18n/vi.ts`
- Modify: `src/lib/i18n/en.ts`

**Interfaces:**
- Produces: a `navigation-hud` window and `Ctrl+Alt+H` action mapped to `navigation.hud_visible`.

- [ ] **Step 1: Write failing hotkey/settings tests**

Assert default HUD visibility, default `Ctrl+Alt+H`, and independent toggle behavior from `minimap.visible`.

- [ ] **Step 2: Run focused Rust tests and verify RED**

Run: `cargo test --lib hotkeys`

- [ ] **Step 3: Implement HUD window creation and supervisor**

Use a transparent, decorations-off, always-on-top, taskbar-hidden, initially hidden window. Set ignore-cursor-events, register it in `win::vis`, anchor to game client top-center, hide when the game is not foreground or the main window is in front, and start only after `hud://ready` (with timeout fallback).

- [ ] **Step 4: Add settings and guide controls**

Expose HUD visibility and hotkey in existing settings/hotkey editors. Keep click-through unconditional for the HUD.

- [ ] **Step 5: Run Rust and frontend verification and commit**

Run: `cargo test --workspace`

Run: `npm test && npm run check && npm run build`

Commit: `git add src-tauri/src src/main src/lib/i18n && git commit -m "feat: supervise click-through compass HUD"`

---

### Task 7: Polling defaults, versioning, documentation, and packaged verification

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Modify: `src/main/dino/DinoTab.svelte`
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src/main/guide/Guide.svelte`
- Create: `docs/navigation-hud-qa.md`

**Interfaces:**
- Produces: distinguishable `1.5.3-nav.1` installer and a concise user QA/recovery guide.

- [ ] **Step 1: Add a settings migration test**

Assert existing 10-second IslePilot settings migrate once to 5 seconds while explicit 5-second settings remain unchanged. Preserve unrelated keys.

- [ ] **Step 2: Implement migration and version bump**

Set package, Cargo, and Tauri versions consistently to `1.5.3-nav.1`. Document that future official updates are optional and user-triggered.

- [ ] **Step 3: Run complete automated verification**

Run: `cargo fmt --all -- --check`

Run: `cargo clippy --workspace --all-targets -- -D warnings`

Run: `cargo test --workspace`

Run: `npm test`

Run: `npm run check`

Run: `npm run build`

Run: `npm run tauri build`

Expected: every command exits 0; NSIS installer and executable exist under `src-tauri/target/release/bundle/nsis` and `src-tauri/target/release`.

- [ ] **Step 4: Perform non-interactive runtime smoke tests**

Launch the new executable only after ensuring it uses a separate temporary profile or after the user permits closing the installed overlay. Verify process survival, main/minimap/HUD window creation, no new error logs, and no settings/waypoint loss. Do not control the game window.

- [ ] **Step 5: Install only at the final gate**

Ask the user to finish the current play session only when automated verification passes. Close the old overlay, back up current settings, install the NSIS package, start it, confirm version/settings/token presence without displaying secrets, then ask the user for one short in-game heading/waypoint check.

- [ ] **Step 6: Final commit and evidence summary**

Run: `git status --short`

Run the complete verification commands again after any installation-only fixes.

Commit: `git add -A && git commit -m "release: navigation HUD custom build"`
