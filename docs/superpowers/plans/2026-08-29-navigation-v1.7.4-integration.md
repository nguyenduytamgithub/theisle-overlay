# Navigation v1.7.4 Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Safely merge deterministic newbie navigation into the current Night Vision/API release, build v1.7.4, preserve user data, and install it without stopping The Isle.

**Architecture:** Preserve both histories with a non-fast-forward merge. Navigation owns estimator and presentation semantics; Rust remains the confirmed-position truth boundary; Night Vision and IslePilot resilience remain independent. Existing branch tests are the integration contract, and any new conflict-resolution behavior requires a failing regression test before production edits.

**Tech Stack:** Git worktrees, TypeScript ES modules, Node test runner, Svelte 5, Rust, Tauri 2, NSIS, PowerShell.

## Global Constraints

- Merge source is exactly `public/navigation-hud` at `21ae583`.
- Pre-merge current branch is `codex/navigation-hud` at `c206501`.
- Release version is `1.7.4` in npm, Cargo, Tauri, and release tests.
- Keep magnifier boost-b and the 8-second IslePilot poll timeout with retry delay capped at 15 seconds.
- Never stop, restart, patch, or write inside The Isle.
- Preserve the encrypted IslePilot token, settings, waypoints, and confirmed trails.
- Do not push or claim live acceptance until local verification and installation evidence exist.

---

### Task 1: Prove both branch baselines

**Files:** No source changes.

- [x] **Step 1:** On `public/navigation-hud`, run `node --test src/lib/navigation/*.test.mjs`; require 34 pass, 0 fail.
- [x] **Step 2:** Run `npm run check`; require zero errors and warnings.
- [x] **Step 3:** Run `cargo test --workspace --manifest-path src-tauri/Cargo.toml`; require 124 non-live tests pass and only 9 documented ignores.
- [ ] **Step 4:** Verify `codex/navigation-hud` is clean, pushed at `c206501`, and remains in the existing linked worktree.

### Task 2: Merge histories and resolve conflicts

**Files:** Conflict-dependent; inspect Git's exact conflict list before editing.

- [ ] **Step 1:** Run `git merge --no-ff --no-commit public/navigation-hud`.
- [ ] **Step 2:** Classify every conflict as navigation-owned, current-release-owned, or combined; record the list in the merge status.
- [ ] **Step 3:** Resolve source conflicts by preserving both independent behaviors. For any novel combined behavior, add and run a failing regression test before editing production code.
- [ ] **Step 4:** Resolve all version files and release assertions to literal `1.7.4` values.
- [ ] **Step 5:** Run `git diff --check`, verify no conflict markers, and commit the merge.

### Task 3: Verify the integrated source

**Files:** Tests and production files included by the merge.

- [ ] **Step 1:** Run all navigation tests; require 34 pass, 0 fail.
- [ ] **Step 2:** Run `npm run check` and `npm run build`; require zero Svelte diagnostics and a successful Vite build.
- [ ] **Step 3:** Run full Rust workspace tests and Clippy `-D warnings` in `D:\CodexBuild\theisle-overlay-navigation-v1.7.4`.
- [ ] **Step 4:** Run Night Vision safety, release configuration, forbidden-API, and secret-shape scans.
- [ ] **Step 5:** Resynchronize CodeGraph and query the combined HUD/navigation/Night Vision lifecycle.

### Task 4: Package and install without touching the game

**Files:** Generated artifacts only under `D:\CodexBuild\theisle-overlay-navigation-v1.7.4`.

- [ ] **Step 1:** Record hashes/metadata for settings, token, waypoints, and trail files without printing secrets.
- [ ] **Step 2:** Build the v1.7.4 NSIS installer and record size, version, and SHA-256.
- [ ] **Step 3:** Verify the exact installed overlay path and PID, stop only that process, run the installer silently, and relaunch the overlay.
- [ ] **Step 4:** Prove the same The Isle PID remains responsive and preservation targets remain unchanged except the documented settings migration.
- [ ] **Step 5:** Verify installed v1.7.4 runtime logs for HUD readiness, navigation state, Night Vision fingerprint, and IslePilot polling.

### Task 5: Publish and hand off honestly

**Files:** Git metadata and public branch only.

- [ ] **Step 1:** Push `codex/navigation-hud` without force and read back the exact remote commit.
- [ ] **Step 2:** Report automated/install status separately from live-game navigation acceptance.
- [ ] **Step 3:** Ask only for the minimum live action: select one waypoint and move across three confirmed coordinate updates.
