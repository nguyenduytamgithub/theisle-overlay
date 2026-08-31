# Fixed-Center Water Guide Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Water Guide ray screen-fixed and give a stable, unambiguous green alignment lock for new players.

**Architecture:** Pure TypeScript navigation helpers define the fixed ray, 8/18-degree hysteresis, and localized prompts. The Water Guide webview stores one alignment bit and renders a fixed center beam plus a separate maneuver banner.

**Final XY-only amendment:** `guidanceCourseDeg` is not a Water Guide input. The webview derives a course only from displacement between accepted confirmed XY samples, while the fixed center ray remains visible even before movement is known. Head/facing and mouse/camera rotation never enter the Water Guide calculation.

**Tech Stack:** TypeScript, Node test runner, Tauri 2, HTML/CSS, Rust verification, NSIS.

## Global Constraints

- Preserve the running The Isle process; restart only the Overlay during installation.
- Do not read game memory, inject or hook rendering, capture packets, or control character movement.
- Keep `Ctrl+Alt+W` and `Ctrl+Alt+N` unchanged.
- Hide the ray for waiting, stale, arrived, and invalid states.

---

### Task 1: Fixed ray and alignment contract

**Files:**
- Modify: `src/lib/navigation/water-guide.ts`
- Test: `src/lib/navigation/water-guide.test.mjs`

**Interfaces:**
- Produces: `nextAlignmentLocked(previous: boolean, frame: WaterGuideFrame): boolean`
- Produces: `steeringPromptFor(frame: WaterGuideFrame, aligned: boolean, language: WaterGuideLanguage): string`
- Changes: every visible `WaterGuideFrame.screenAngleDeg` is `0`

- [x] **Step 1: Write failing tests** for a vertical ray under left/right/U-turn headings, 8/18-degree lock hysteresis, and Vietnamese left/right/locked prompts.
- [x] **Step 2: Verify RED** with `node --experimental-strip-types --test src/lib/navigation/water-guide.test.mjs`.
- [x] **Step 3: Implement the pure helpers** using `alignEnterDeg: 8` and `alignExitDeg: 18`, with rounded degree copy.
- [x] **Step 4: Verify GREEN** with the same targeted Node command.

### Task 2: Fixed-center renderer and green lock

**Files:**
- Modify: `src/water-guide/main.ts`
- Modify: `water-guide.html`
- Modify: `src/water-guide/style.css`

**Interfaces:**
- Consumes: `nextAlignmentLocked` and `steeringPromptFor`
- Maintains: one module-level `alignmentLocked` boolean, reset whenever the ray is hidden

- [x] **Step 1: Remove rotational paint state** and always set `--ray-angle` to `0deg`.
- [x] **Step 2: Render maneuver state** into `#maneuver`, set `data-aligned`, and reset the lock on hidden/error states.
- [x] **Step 3: Add locked styling** so the center ray and maneuver banner become green only while aligned.
- [x] **Step 4: Run frontend gates**: all navigation tests, `npm run check`, and `npm run build`.

### Task 3: Package, live acceptance, and integration

**Files:**
- Modify: `docs/verification/water-guide-ray-live.md`
- Modify: `docs/superpowers/plans/2026-08-30-water-guide-ray.md`

**Interfaces:**
- Produces: installed v1.9.0 executable and NSIS hashes plus live screenshot evidence

- [x] **Step 1: Run Rust gates**: `cargo test --manifest-path src-tauri/Cargo.toml --lib` and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [x] **Step 2: Build NSIS** with `CARGO_TARGET_DIR=D:\CodexBuild\theisle-overlay-water-guide-target`.
- [x] **Step 3: Install while preserving the game PID**, then verify the installed executable hash.
- [x] **Step 4: Live-check** fixed ray, left/right prompt, green lock, and Night Vision stacking; capture evidence.
- [ ] **Step 5: Update evidence docs, commit, push, and merge** into the verified base branch selected from Git history/remotes.
