# Water Guide North-Up XY Board Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the camera-glued Water Guide ray with a large north-up XY navigation board rendered directly over The Isle game window.

**Architecture:** Keep the verified Rust freshwater route and full-game click-through window unchanged. Extend the pure TypeScript Water Guide geometry with board-needle output, then replace only the dedicated Water Guide HTML/CSS renderer. Target and movement needles are absolute north-up bearings derived from confirmed XY; mouse/camera/head/server-facing data never enters this path.

**Tech Stack:** TypeScript 5, Node test runner, Vite 7, Tauri 2, Rust 2021, CSS.

## Global Constraints

- The board is drawn by the dedicated `water-guide` full-game overlay, not by the minimap or big map.
- `Ctrl+Alt+W` remains the Water Guide toggle and `Ctrl+Alt+N` remains Night Vision.
- North is always board-up; east/right, south/down, and west/left.
- Target and movement needles may change only after accepted XY evidence changes.
- Movement course requires at least 500 cm confirmed displacement.
- Do not consume server-facing, character-head, camera yaw/pitch/FOV, view matrices, mouse input, game memory, injected hooks, packets, continuous game capture, or synthetic game input.
- No CSS animation or rotation transition.
- While Water Guide is requested, keep the ordinary heading HUD and rotating minimap suppressed; restore both when it is off.
- Preserve the running The Isle process; install/restart only Overlay.
- Live visual acceptance is required before merge or public push.

---

### Task 1: Pure north-up board geometry

**Files:**
- Modify: `src/lib/navigation/water-guide.ts`
- Modify: `src/lib/navigation/water-guide.test.mjs`

**Interfaces:**
- Consumes: `WaterGuideFrame`, current `movementCourseDeg`, existing `desiredBearingDeg`, and `rayVisible` state.
- Produces: `WaterGuideBoardNeedles` and `waterGuideBoardNeedles(frame, movementCourseDeg)`.

- [x] **Step 1: Write the failing needle tests**

Add tests proving north/east/south/west target bearings map to 0/90/180/270, movement unknown hides only the movement needle, waiting preserves both known absolute bearings, and arrived/invalid hides both needles.

```javascript
const board = waterGuideBoardNeedles(frame, 270);
assert.equal(board.targetBearingDeg, 90);
assert.equal(board.movementBearingDeg, 270);
assert.equal(board.targetVisible, true);
assert.equal(board.movementVisible, true);
```

- [x] **Step 2: Run RED**

Run:

```powershell
node --test src/lib/navigation/water-guide.test.mjs
```

Expected: module import or function assertions fail because `waterGuideBoardNeedles` does not exist.

- [x] **Step 3: Implement the minimal pure board contract**

```typescript
export interface WaterGuideBoardNeedles {
  targetBearingDeg: number | null;
  movementBearingDeg: number | null;
  targetVisible: boolean;
  movementVisible: boolean;
}

export function waterGuideBoardNeedles(
  frame: WaterGuideFrame,
  movementCourseDeg: number | null,
): WaterGuideBoardNeedles;
```

Normalize both finite bearings into `[0, 360)`. Target visibility follows a valid visible guide frame. Movement visibility additionally requires a finite movement course. Arrival and invalid states hide both.

- [x] **Step 4: Run GREEN and the complete navigation suite**

```powershell
node --test src/lib/navigation/water-guide.test.mjs
node --test src/lib/navigation/*.test.mjs
```

Expected: zero failures.

- [x] **Step 5: Commit**

```powershell
git add -- src/lib/navigation/water-guide.ts src/lib/navigation/water-guide.test.mjs
git commit -m "feat: derive north-up water guide needles"
```

### Task 2: Replace the ray with an on-screen XY board

**Files:**
- Modify: `water-guide.html`
- Modify: `src/water-guide/main.ts`
- Modify: `src/water-guide/style.css`
- Modify: `src/lib/navigation/water-guide-style.test.mjs`

**Interfaces:**
- Consumes: `waterGuideBoardNeedles`, existing route/position events, and the existing full-game Water Guide window.
- Produces: fixed compass board DOM elements `board`, `target-needle`, and `movement-needle`; CSS variables `--target-bearing` and `--movement-bearing`.

- [x] **Step 1: Write the failing renderer contract tests**

Read the renderer source, stylesheet, and HTML in `water-guide-style.test.mjs`. Assert:

```javascript
assert.match(html, /id="board"/);
assert.match(html, /id="target-needle"/);
assert.match(html, /id="movement-needle"/);
assert.match(main, /waterGuideBoardNeedles/);
assert.match(main, /--target-bearing/);
assert.match(main, /--movement-bearing/);
assert.doesNotMatch(main, /--ray-angle/);
assert.doesNotMatch(css, /animation\s*:|@keyframes|transition\s*:[^;]*transform/);
assert.doesNotMatch(html, /id="ray"/);
```

- [x] **Step 2: Run RED**

```powershell
node --test src/lib/navigation/water-guide-style.test.mjs
```

Expected: fails because the old ray DOM exists and the board DOM/variables do not.

- [x] **Step 3: Replace the HTML with the board**

Create one lower-middle navigation panel containing a 300 px compass disc, four full Vietnamese cardinal labels, a cyan target needle, a white/green movement needle, centre dot, and short legend. Remove the vertical ray and eight chevrons.

- [x] **Step 4: Wire absolute bearings in the renderer**

Call `waterGuideBoardNeedles(frame, movementCourseDeg)`. Set `--target-bearing` and `--movement-bearing` only from its returned absolute XY values. Toggle needle visibility separately and keep stale state through `data-state`. Never read pointer/mouse/camera/heading fields.

- [x] **Step 5: Style the non-animated full-game board**

Use a translucent lower-middle disc that does not cover the top destination/status row. Rotate each needle around the board centre from `transform-origin: 50% 100%`; do not add animation or transform transitions. Use cyan for the target, white for movement, green for aligned movement, and amber text for corrections.

- [x] **Step 6: Run GREEN and frontend gates**

```powershell
node --test src/lib/navigation/water-guide-style.test.mjs
node --test src/lib/navigation/*.test.mjs
npm run check
npm run build
git diff --check
```

Expected: all tests pass, Svelte has 0 errors/0 warnings, and Vite emits `dist/water-guide.html`.

- [x] **Step 7: Commit**

```powershell
git add -- water-guide.html src/water-guide/main.ts src/water-guide/style.css src/lib/navigation/water-guide-style.test.mjs
git commit -m "feat: render north-up XY board over game"
```

### Task 3: Review, package, install, and live acceptance

**Files:**
- Modify: `docs/verification/water-guide-ray-live.md`
- Modify: `docs/superpowers/plans/2026-08-31-water-guide-xy-board.md`

**Interfaces:**
- Consumes: verified source branch, existing NSIS build path, and the active The Isle session.
- Produces: independently reviewed commit, installer/EXE hashes, two live captures, and a clean Git candidate.

- [x] **Step 1: Synchronize CodeGraph and review blast radius**

Run the user-scope ensure script and one `codegraph explore --max-files 2` query for Water Guide renderer consumers. Confirm no camera, mouse, character-control, memory, hook, or packet source entered the feature.

- [x] **Step 2: Run complete automated verification**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-water-guide-target'
node --test src/lib/navigation/*.test.mjs
npm run check
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
git diff --check
```

Expected: zero failures or warnings other than the existing Vite chunk-size notice.

- [x] **Step 3: Obtain independent code review**

Review pure geometry, absolute-bearing rendering, no-camera contract, stale/arrival behavior, and HUD/minimap suppression. Fix any finding with a new failing regression before proceeding. Require `READY`.

- [x] **Step 4: Build and install only Overlay**

```powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-water-guide-target'
npm run tauri build -- --bundles nsis
```

Record installer and installed EXE SHA-256. Back up the installed EXE under `D:\CodexBuild`, stop/restart only `theisle-overlay.exe`, and verify the existing The Isle PID/start time are unchanged.

- [ ] **Step 5: Capture live camera-independence evidence**

With Water Guide requested, capture two 1920 x 1080 desktop frames after the user changes only the camera angle. Require the same target/movement needle angles when no new confirmed XY sample arrived; require no minimap/ordinary heading HUD and visible on-screen cardinal labels. Do not send input to the game.

Handoff note: active-game and Night Vision visual captures passed, but the mouse-only camera pair was not executed because no synthetic game input is allowed and the user requested handoff without reopening the game. Absolute-XY/no-camera behavior passed source inspection and deterministic regressions.

- [x] **Step 6: Finalize docs and Git**

Record screenshot paths/hashes, installer/EXE hashes, test counts, live `PASS/PARTIAL/BLOCKED`, and running PIDs. Mark this plan complete only after live acceptance. Commit docs, require clean status, inspect worktree ancestry, then merge/push without force and without touching other worktrees.
