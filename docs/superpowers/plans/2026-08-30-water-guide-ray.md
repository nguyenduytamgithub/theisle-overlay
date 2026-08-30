# Water Guide Ray Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `Ctrl+Alt+W` Water Guide that locks the nearest verified shallow freshwater destination and renders a simple, honest, course-relative blue guidance ray over The Isle.

**Architecture:** Rust owns freshwater-mask validation, shallow-shore selection, locked route state, hotkey dispatch, and the click-through game-window overlay lifecycle. A pure TypeScript module projects the current estimated position onto the locked segment, chooses a catch-up point, and derives the screen cue; a dedicated minimal Vite entry renders from the existing position events without reading or controlling the game.

**Tech Stack:** Rust 2021, Tauri 2, `image` PNG decoding, `sha2`, TypeScript 5, Vite 7, Node test runner, Svelte 5, and the project's existing safe Win32 window/hotkey APIs.

## Global Constraints

- Default toggle is exactly `Ctrl+Alt+W`; W means Water.
- Lock endpoints A and B for one activation; only off/on creates a new route.
- Select B from `freshwater.png` pixels with alpha >= 128, near the shallow boundary and never from ocean, an arbitrary blue pixel, or a raw POI centre.
- Use the nearest `water` POI only as the readable label.
- Fail closed when the freshwater asset, POI map version, image geometry, position, or calibration cannot be verified.
- Guidance is 2D and player-controlled: no game-memory access, injection/hooks, packet capture, continuous game capture, synthetic input, automated movement, or obstacle avoidance.
- Motion-derived course is preferred; stable server-facing is the fallback; unknown/stale evidence must not appear confident.
- Initial thresholds are: on-route 15 m, badly lost 150 m, arrival 25 m, look-ahead 80 m, U-turn 110 degrees.
- Keep the Night Vision branch and running game untouched; install/restart only the Overlay when required.
- A green build is not live acceptance: capture the ray over the actual game window and verify its destination against freshwater data.
- Publish this feature as version `1.9.0`, with package, Rust, Tauri, and lockfile versions synchronized.

---

## File Structure

- Create `src-tauri/src/water_guide.rs` for asset identity/cache, boundary extraction, POI labels, locked route state, commands, and events.
- Create `src-tauri/src/water_guide_window.rs` for transparent full-client window construction, focus visibility, resize/reposition, recovery, and topmost supervision.
- Create `src/lib/navigation/water-guide.ts` and `water-guide.test.mjs` for pure route geometry and direction state.
- Create `src/water-guide/main.ts` and `style.css` plus `water-guide.html` for the dedicated minimal renderer.
- Modify `src-tauri/Cargo.toml`, `Cargo.lock`, `state.rs`, `lib.rs`, `hotkeys.rs`, `settings.rs`, capabilities, Vite inputs, settings UI, guide, i18n, README, and HUONG_DAN.
- Create `docs/verification/water-guide-ray-live.md` for hashes, test counts, live screenshots, and honest acceptance status.

### Task 1: Freshwater mask and shallow-shore target engine

**Files:**
- Create: `src-tauri/src/water_guide.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `settings::islemaps_dir()`, `settings::pois_path()`, `fetch::MAP_VERSION`, and `overlay_core::{pixel_to_world, distance_m, Calibration}`.
- Produces: `FreshwaterTarget`, `FreshwaterCache`, `WaterGuideError`, and `select_freshwater_target`.

- [x] **Step 1: Write failing synthetic-mask tests**

Add tests inside `water_guide.rs` before production functions. They construct real `image::RgbaImage` values without mocks:

~~~rust
#[test]
fn transparent_ocean_is_never_a_candidate() {
    let mut mask = image::RgbaImage::new(7, 7);
    for y in 2..=4 {
        for x in 2..=4 {
            mask.put_pixel(x, y, image::Rgba([0, 140, 255, 255]));
        }
    }
    let candidates = shallow_candidates(&mask);
    assert!(!candidates.is_empty());
    assert!(candidates.iter().all(|&(x, y)| mask.get_pixel(x, y).0[3] >= 128));
    assert!(!candidates.contains(&(0, 0)));
}

#[test]
fn boundary_moves_one_pixel_toward_denser_freshwater() {
    let mask = square_mask(7, 2..=4, 2..=4);
    let candidates = shallow_candidates(&mask);
    assert!(candidates.contains(&(3, 3)));
    assert!(!candidates.contains(&(1, 3)));
}
~~~

Also add a hand-derived nearest-world-distance test using a 10 x 10 calibration over world ranges 0..10, candidates `[(1,1), (8,8)]`, player `(1_500 cm, 1_200 cm)`, and expected candidate `(1,1)`.

- [x] **Step 2: Run tests and verify RED**

Run:

~~~powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-water-guide-target'
cargo test --manifest-path src-tauri/Cargo.toml water_guide --lib
~~~

Expected: compilation fails because the module/functions and shipped `image` dependency do not exist.

- [x] **Step 3: Add the shipped image dependency and minimal mask geometry**

Change:

~~~toml
[features]
devtools = []

image = { version = "0.25", default-features = false, features = ["png", "webp"] }
~~~

Implement:

~~~rust
const WATER_ALPHA_MIN: u8 = 128;
fn is_water(mask: &image::RgbaImage, x: u32, y: u32) -> bool;
fn water_neighbour_count(mask: &image::RgbaImage, x: u32, y: u32) -> u8;
fn inset_from_boundary(mask: &image::RgbaImage, x: u32, y: u32) -> (u32, u32);
pub(crate) fn shallow_candidates(mask: &image::RgbaImage) -> Vec<(u32, u32)>;
pub(crate) fn nearest_candidate(
    candidates: &[(u32, u32)],
    player_x_cm: f64,
    player_y_cm: f64,
    calibration: &Calibration,
) -> Option<(u32, u32)>;
~~~

Find water pixels adjacent to non-water/out-of-bounds, move each to the adjacent water pixel with greatest neighbour count when denser, deduplicate with `BTreeSet`, and keep deterministic ordering.

- [x] **Step 4: Run focused tests and verify GREEN**

Expected: all Water Guide geometry tests pass with no warnings.

- [x] **Step 5: Write failing data-validation and POI-label tests**

Use a literal JSON fixture:

~~~rust
#[test]
fn poi_label_does_not_replace_the_mask_destination() {
    let pois = serde_json::json!({
        "map": "Gateway_v0.21.7",
        "layers": {"water": {"items": [
            {"label": "Near Pond", "x": 10_000.0, "y": 20_000.0},
            {"label": "Far Pond", "x": 500_000.0, "y": 500_000.0}
        ]}}
    });
    assert_eq!(nearest_water_label(&pois, 11_000.0, 19_000.0).unwrap(), "Near Pond");
}
~~~

Add tests rejecting the wrong map version, image-size mismatch, an empty mask, and missing labels.

- [x] **Step 6: Run tests and verify RED**

Expected: missing `nearest_water_label`, asset identity/cache, and selector failures.

- [x] **Step 7: Implement validation, caching, and selection**

Use these contracts:

~~~rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct AssetIdentity { len: u64, modified_ns: u128, sha256: String }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreshwaterTarget {
    pub label: String,
    pub x_cm: f64,
    pub y_cm: f64,
    pub mask_px: [u32; 2],
    pub distance_m: f64,
}

pub fn select_freshwater_target(
    cache: &mut FreshwaterCache,
    player_x_cm: f64,
    player_y_cm: f64,
) -> Result<FreshwaterTarget, WaterGuideError>;
~~~

Require dimensions equal `Calibration::islemaps()`, hash with existing `sha2`, require POI `map == fetch::MAP_VERSION`, convert candidates through `pixel_to_world`, select by world-centimetre distance, and attach only the nearest `layers.water.items[].label`. Stable errors: `missing_freshwater`, `invalid_freshwater`, `unsupported_map`, `empty_freshwater`, `missing_water_labels`.

- [x] **Step 8: Verify focused and complete Rust tests**

Run focused Water Guide tests followed by `cargo test --manifest-path src-tauri/Cargo.toml --workspace --all-targets`. Expected: zero failures.

- [x] **Step 9: Commit**

~~~powershell
git add -- src-tauri/src/water_guide.rs src-tauri/src/lib.rs src-tauri/Cargo.toml Cargo.lock
git commit -m "feat: select verified shallow freshwater targets"
~~~

### Task 2: Locked state, Tauri command, and Ctrl+Alt+W

**Files:**
- Modify: `src-tauri/src/water_guide.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/hotkeys.rs`
- Modify: `src-tauri/src/settings.rs`

**Interfaces:**
- Consumes: `pipeline::current_payload(&AppState)`, `events::emit_all`, and existing hotkey registration/debounce.
- Produces: `WaterGuideSnapshot`, `WaterGuideRoute`, commands `get_water_guide_state` and `toggle_water_guide`, event `water-guide://changed`, and `toggle_from_app`.

- [x] **Step 1: Write failing route-lock and settings tests**

Test a pure runtime with an injected selector closure: first on stores A/B; reading snapshots cannot change them; off clears; next on locks from the newer supplied position. Add a test that missing, out-of-bounds, or older-than-30-seconds position evidence leaves `requested=true`, `route=None`, and `errorKey=waiting_for_position`. Extend `merge_real_legacy_settings_loses_nothing` with:

~~~rust
assert_eq!(merged["hotkeys"]["toggle_water_guide"], "Ctrl+Alt+W");
~~~

- [x] **Step 2: Run focused tests and verify RED**

Run focused Water Guide and legacy-settings tests. Expected: missing state/contracts/default binding.

- [x] **Step 3: Implement route state and commands**

Use exact serialized contracts:

~~~rust
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WaterGuideRoute {
    pub start_x_cm: f64,
    pub start_y_cm: f64,
    pub target_x_cm: f64,
    pub target_y_cm: f64,
    pub target_mask_px: [u32; 2],
    pub label: String,
    pub initial_distance_m: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WaterGuideSnapshot {
    pub requested: bool,
    pub route: Option<WaterGuideRoute>,
    pub error_key: Option<String>,
}
~~~

Add `pub water_guide: Mutex<WaterGuideRuntime>` to `AppState`. On off, clear requested/route/error. On enable, keep `requested=true`; lock one route when valid or retain `route=None` with a stable error key so no ray is drawn. Both hotkey and command call one internal toggle and emit the same snapshot.

- [x] **Step 4: Add default and dispatch**

Add `toggle_water_guide: Ctrl+Alt+W` to defaults. Dispatch through `crate::water_guide::toggle_from_app(app)` and add `water-guide` to recovery reload labels. It remains non-repeatable under the existing 350 ms debounce.

- [x] **Step 5: Verify and commit**

Run focused tests, Cargo fmt check, and full workspace tests; then commit:

~~~powershell
git add -- src-tauri/src/water_guide.rs src-tauri/src/state.rs src-tauri/src/lib.rs src-tauri/src/hotkeys.rs src-tauri/src/settings.rs
git commit -m "feat: lock water routes behind Ctrl Alt W"
~~~

### Task 3: Pure fixed-route guidance

**Files:**
- Create: `src/lib/navigation/water-guide.ts`
- Create: `src/lib/navigation/water-guide.test.mjs`
- Modify: `tsconfig.json` (permit explicit `.ts` imports for Node tests in this no-emit Vite project)

**Interfaces:**
- Consumes: projected navigation position/course/freshness and locked route fields.
- Produces: `projectToSegment`, `waterGuideFrame`, and `WaterGuideFrame`.

- [x] **Step 1: Write failing literal geometry tests**

Cover: 100 m off-route yields an 80 m-ahead catch-up point, 180 degrees yields U-turn, stale/headingless input hides the ray, on-route, 15 m/150 m/25 m boundaries, segment endpoints, zero-length route, north wraparound, left/right sign, and arrows pointing away from origin.

Representative assertion:

~~~javascript
const frame = waterGuideFrame(
  route({ startXCm: 0, startYCm: 0, targetXCm: 100_000, targetYCm: 0 }),
  view({ xCm: 20_000, yCm: 10_000, guidanceCourseDeg: 0, freshness: "tracking" }),
);
assert.equal(Math.round(frame.crossTrackM), 100);
assert.deepEqual(frame.steeringTargetCm, [28_000, 0]);
assert.equal(frame.state, "off-route");
~~~

- [x] **Step 2: Run `node --test src/lib/navigation/water-guide.test.mjs` and verify RED**

Expected: module-not-found.

- [x] **Step 3: Implement minimal pure guidance**

~~~typescript
export const WATER_GUIDE = {
  onRouteM: 15, lostM: 150, arrivalM: 25, lookAheadM: 80, uturnDeg: 110,
} as const;

export type WaterGuideState =
  "on-route" | "off-route" | "lost" | "waiting" |
  "heading-unknown" | "arrived" | "invalid";

export interface WaterGuideFrame {
  state: WaterGuideState;
  rayVisible: boolean;
  steeringTargetCm: [number, number] | null;
  remainingM: number;
  crossTrackM: number;
  desiredBearingDeg: number | null;
  relativeDeg: number;
  turn: "left" | "right" | "straight" | "uturn" | "none";
}
~~~

Use existing `bearingTo` and `relativeBearing`. If cross-track exceeds 15 m, advance 8,000 cm from the clamped projection toward B; otherwise steer directly to B. Never emit a ray for stale, headingless, invalid, or arrived states.

- [x] **Step 4: Verify GREEN and mutation check**

Run focused then all navigation tests. Temporarily change look-ahead to 79, confirm the literal test fails, restore 80, and confirm green.

- [x] **Step 5: Commit**

~~~powershell
git add -- src/lib/navigation/water-guide.ts src/lib/navigation/water-guide.test.mjs
git commit -m "feat: derive stable fixed-route water guidance"
~~~

### Task 4: Click-through game window and renderer

**Files:**
- Create: `src-tauri/src/water_guide_window.rs`
- Create: `water-guide.html`
- Create: `src/water-guide/main.ts`
- Create: `src/water-guide/style.css`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `vite.config.ts`

**Interfaces:**
- Consumes: state command/event, position/quality/settings events, `NavigationEstimator`, and `waterGuideFrame`.
- Produces: `water-guide://ready` and a transparent window labelled `water-guide` covering the game client rectangle.

- [x] **Step 1: Write failing window-policy tests**

Test `should_show(requested, game_active, main_in_front)`, exact client rectangle placement/sizing, one-recreate/then-supervise recovery, and capability membership for `water-guide`.

- [x] **Step 2: Run focused Rust tests and verify RED**

Expected: missing module/functions/capability.

- [x] **Step 3: Implement the supervised window**

Build `water-guide.html` as transparent, decorationless, shadowless, topmost, taskbar-skipped, non-resizable, unfocused, non-focusable, initially hidden, and `set_ignore_cursor_events(true)`. Copy the proven HUD lifecycle: 250 ms supervisor, two-observation game debounce, hide on Alt-Tab or main-window foreground, resize/reposition to `client_rect_on_screen`, topmost repair, resync on show, and one ready recreation.

- [x] **Step 4: Add Vite entry and renderer wiring**

Add the Vite input and capability. Feed `NavigationEstimator` from real position events, invalidate on quality reset, update state from `water-guide://changed`, invoke initial state/position, paint at 30 FPS, and emit `water-guide://ready`. Compute `waterGuideFrame` and set CSS variables for a bottom-centre ray; never display the ray when `rayVisible=false`.

- [x] **Step 5: Implement Vietnamese-first visuals**

Render a cyan line/glow with repeated outward chevrons, compact destination/distance pill, and exact instructions:

- `THEO TIA XANH`
- `LỆCH ĐƯỜNG · QUAY LẠI TIA XANH`
- `LẠC XA · THEO TIA XANH ĐỂ TRỞ LẠI`
- `QUAY ĐẦU`
- `CHỜ SERVER`
- `XOAY / ĐI VÀI BƯỚC ĐỂ XÁC ĐỊNH HƯỚNG`
- `ĐÃ TỚI NGUỒN NƯỚC`
- `KHÔNG XÁC MINH ĐƯỢC NƯỚC UỐNG`

Map error keys locally and never display raw backend errors.

- [x] **Step 6: Verify and commit**

Run focused Rust window tests, all navigation tests, `npm run check`, and `npm run build`. Require `dist/water-guide.html`. Commit window/config/renderer files with message `feat: render water guidance over the game`.

### Task 5: Settings, help, diagnostics, and documentation

**Files:**
- Modify: `src/main/settings/HotkeyEditor.svelte`
- Modify: `src/main/guide/Guide.svelte`
- Modify: `src/lib/i18n/vi.ts`
- Modify: `src/lib/i18n/en.ts`
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `README.md`
- Modify: `HUONG_DAN.md` if present
- Modify: `src-tauri/src/water_guide.rs`
- Create: `docs/verification/water-guide-ray-live.md`

**Interfaces:**
- Consumes: the settings action and Water Guide state transitions.
- Produces: discoverable rebinding, concise usage/recovery instructions, safe diagnostics, and a live evidence record.

- [ ] **Step 1: Add the action and translations**

Place `toggle_water_guide` after `toggle_hud` in both action arrays. Add Vietnamese `Bật/tắt chỉ đường tới nước` and English `Toggle Water Guide` labels.

- [ ] **Step 2: Add concise help**

Explain: press Ctrl+Alt+W, follow blue arrows, yellow QUAY ĐẦU means reverse, off/on locks a new nearest freshwater route, and the feature does not avoid terrain or drive the character.

- [ ] **Step 3: Add safe diagnostics**

Log only requested/result/error key, target label, rounded initial distance, candidate count, mask dimensions/hash, and map version. Never log Steam tokens, cookies, authenticated URLs, session payloads, or continuous coordinate history.

- [ ] **Step 4: Add public docs and evidence table**

Document the improvement, use, data/freshness limitations, upstream attribution, and EAC-safe boundary. Create evidence rows for commit, executable/data hashes, selected label/pixel/world coordinate, toggle, game screenshot, Alt-Tab hide, U-turn/heading state, and user-controlled drinking confirmation.

- [ ] **Step 5: Synchronize version 1.9.0**

Set `version` to `1.9.0` in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`. Run `npm install --package-lock-only --ignore-scripts` and `cargo metadata --manifest-path src-tauri/Cargo.toml --no-deps` to prove both lock ecosystems resolve the same application version.

- [ ] **Step 6: Verify and commit**

Run frontend check/build, `git diff --check`, a red-flag marker scan, and credential-like string scan. Commit UX/docs/version files with `docs: explain safe water guide controls`.

### Task 6: Full verification, packaging, installation, and live proof

**Files:**
- Modify: `docs/verification/water-guide-ray-live.md`
- Modify only for a reproduced live defect: the smallest test/source files that demonstrate and fix it.

**Interfaces:**
- Consumes: complete branch and installed POI/freshwater assets.
- Produces: fresh automated evidence, NSIS installer, installed executable, live screenshots, independent target cross-check, and a clean published branch.

- [ ] **Step 1: Synchronize CodeGraph and inspect final blast radius**

Run the user-scope ensure script, then one focused `codegraph_explore` with `maxFiles: 2` for Water Guide state/hotkey/window consumers. Confirm no unexpected path owns character input or game memory.

- [ ] **Step 2: Run the full verification matrix**

~~~powershell
$env:CARGO_TARGET_DIR='D:\CodexBuild\theisle-overlay-water-guide-target'
node --test src/lib/navigation/*.test.mjs
npm run check
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml --workspace --all-targets
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings
git diff --check
~~~

Scan tracked source for process-memory access, injection/hooks, packet capture, `SendInput`/key simulation, low-level keyboard hooks, and character-control loops. Manually review safe `RegisterHotKey` and boundary-document matches.

- [ ] **Step 3: Build the NSIS installer**

Run `npm run tauri build -- --bundles nsis` with the dedicated Cargo target. Record installer path, size, SHA-256, and executable SHA-256. Build success alone is packaging evidence.

- [ ] **Step 4: Install without interrupting The Isle**

Record The Isle and Overlay PIDs. Back up the installed Overlay/version under `D:\CodexBuild`. Stop only `theisle-overlay.exe`, install the current-user NSIS package, restart Overlay, and verify The Isle PID remained running.

- [ ] **Step 5: Verify target independently**

Record current map version, image dimensions, and POI/mask hashes. After a real activation, independently confirm `targetMaskPx` has alpha >= 128, lies near a freshwater boundary, round-trips to the route world coordinate, and uses the nearest water POI label—not ocean/non-water.

- [ ] **Step 6: Capture live game proof**

With The Isle foreground, toggle through normal user-level interaction, capture the actual game window showing the ray/status, observe correct direction/U-turn/waiting when available, and verify Alt-Tab hides it. Do not synthesize continuous character movement or bypass anti-cheat.

- [ ] **Step 7: Fix reproduced defects through TDD**

For every observed defect, first add a failing regression, observe the expected failure, implement the smallest fix, rerun focused and complete verification, then commit once.

- [ ] **Step 8: Finalize evidence and Git state**

Populate evidence with timestamps, hashes, test counts, screenshots, and honest PASS/PARTIAL/BLOCKED per criterion. Commit evidence; require empty `git status --short` and record feature history.

- [ ] **Step 9: Publish verified work**

After live visual evidence, push `codex/water-guide-ray-v1` to the user's GitHub remote, preserve the upstream source attribution, and hand over exact use/install/rollback steps. Never force-push or rewrite unrelated history.
