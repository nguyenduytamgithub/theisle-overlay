# Water Guide Ray — Design Specification

- Date: 2026-08-30
- Status: proposed for owner approval
- Target branch: `codex/water-guide-ray-v1`
- Base revision: `a431b12bf153f16314a17b7214afa89e7dfa7a6e`

## 1. Goal

Add a beginner-friendly Water Guide that points the player toward the nearest known drinkable freshwater source while The Isle is running. The player toggles it with `Ctrl+Alt+W`. The feature draws a highly visible blue guidance ray directly over the game window, with repeated forward arrows and simple Vietnamese instructions.

The route is player-controlled guidance only. It never reads game memory, injects into the game, captures packets, synthesizes movement input, steers the character, or claims to avoid terrain.

## 2. Locked product decisions

1. `Ctrl+Alt+W` toggles Water Guide on and off. `W` means Water. This combination is not used by the current Overlay defaults and was successfully reserved and released with a live Win32 `RegisterHotKey` probe on this machine.
2. Turning Water Guide on locks one route segment from the activation position `A` to the selected freshwater destination `B`.
3. `A` and `B` do not silently change during that session. Turning the feature off and on again creates a new route from the latest confirmed position.
4. The world route is fixed, but its 2D screen cue updates as the player's estimated course changes. A ray frozen to one screen angle would become incorrect after the player turns.
5. Repeated arrows point away from the bottom-center origin toward the current steering cue. A large yellow U-turn cue appears when the player is facing approximately backward.
6. When the player strays from the original line, guidance first aims toward a catch-up point on that same line and then toward the destination. This makes it easy to recover without silently recalculating a different destination.
7. The Overlay does not promise obstacle avoidance, a camera-aware 3D line, automated walking, or an always-real-time game position. It uses the freshest safe position/course evidence available to the existing Overlay.

## 3. Freshwater evidence and local data contract

The feature distinguishes freshwater from ocean or other non-drinkable water by using a dedicated freshwater mask, not a generic water color or a named POI center.

External cross-checks:

- Vulnona documents a `Drinking Water` overlay for water that is good to drink: <https://www.vulnona.com/game/map/howto.html>
- Vulnona current map: <https://vulnona.com/game/map/>
- IsleDB independently separates `Drinkable Water` from `Undrinkable Water`: <https://isledb.cc/map>
- IsleMaps current Gateway map: <https://islemaps.com/>

Installed runtime evidence at design time:

- POI file: `%LOCALAPPDATA%\TheIsleOverlay\pois_gateway.json`
- Map version: `Gateway_v0.21.7`, schema version `4`
- Freshwater-labelled POIs: `27`
- POI SHA-256: `22A5119C91E305A3FACBAA9F60244516B00115A267A9E7D65DA6EC0E8E7CFFBF`
- Freshwater mask: `%LOCALAPPDATA%\TheIsleOverlay\basemap\islemaps\freshwater.png`
- Mask dimensions: `2500 × 2500`, RGBA
- Mask SHA-256: `6C416181C818C46912C8345C0E183990915B6E7489FDDD06D22D71A79130E05A`
- Opaque freshwater coverage (`alpha >= 128`): `178,368` pixels (`2.8539%`)
- IsleMaps calibration: `x ∈ [-618, 616]`, `y ∈ [-560, 674]`, about `4.94 m/pixel`

The hashes are test evidence for the installed snapshot, not permanent identifiers. A changed valid data asset must be revalidated rather than rejected only because its hash changed.

## 4. Destination selection

### 4.1 Candidate construction

1. Decode `freshwater.png` as RGBA.
2. A pixel is freshwater when `alpha >= 128`.
3. A candidate is a freshwater pixel with at least one of its eight neighbours outside the freshwater mask.
4. Move the chosen boundary candidate one valid pixel inward when possible. At the current calibration this is approximately five metres inside the freshwater shape: close to shore, not the deep centre.
5. Convert candidate pixels to world coordinates using the existing IsleMaps calibration.
6. Cache candidate geometry by asset identity (path, dimensions, modification time and content hash). Invalidate the cache whenever those values change.

### 4.2 Nearest valid destination

Given the latest confirmed player position, choose the nearest boundary candidate by Euclidean world distance. Use the nearest `water` POI only as a human-readable label; the POI centre is not the navigation destination.

The selection must fail closed if the freshwater mask is missing, undecodable, geometrically empty, incompatible with the active map calibration, or stale relative to an unsupported map. It must never fall back to an ocean pixel, arbitrary blue image pixel, generic map centre, or raw POI centre.

## 5. Fixed-route guidance model

When enabled:

- `A`: latest confirmed player world position.
- `B`: selected shallow freshwater boundary point.
- `P`: latest current player world position.
- `S`: nearest projection of `P` onto segment `AB`.
- `e`: cross-track distance `|P - S|`.
- `Q`: look-ahead point on segment `AB`, nominally 80 metres beyond `S` toward `B`, clamped to `B`.

Recommended initial thresholds, exposed as named constants and covered by tests:

- on route: `e <= 15 m`
- off route: `e > 15 m`
- badly lost: `e > 150 m`
- arrived: distance to `B <= 25 m`

Steering target:

- If on route, aim at `B` unless the long-range cue becomes unstable; `Q` may be used as the stabilized forward target.
- If off route, aim at `Q` so the player rejoins the original segment without reversing toward `A`.
- If arrived, remove the directional ray and show `ĐÃ TỚI NGUỒN NƯỚC`.
- No new `B` is selected until the user toggles Water Guide off and on again.

## 6. Screen guidance

The frontend reuses the existing `NavigationEstimator` guidance course: motion-derived course is preferred; stable server-facing is the fallback. It computes the shortest signed relative angle between the bearing to the steering target and the estimated course.

Visual rules:

1. The ray originates near the bottom centre of the game client area and extends toward the relative steering angle.
2. Blue chevrons repeat along the ray and point outward, making a 180-degree mistake visually obvious.
3. When the absolute relative angle exceeds `110°`, show a large yellow `QUAY ĐẦU` cue with the shortest turn direction.
4. Show destination label, remaining distance, route status and data freshness in concise Vietnamese.
5. When the course is unknown, do not invent a heading. Show `XOAY / ĐI VÀI BƯỚC ĐỂ XÁC ĐỊNH HƯỚNG`.
6. When position evidence is stale, freeze the last honest route cue, visibly mark `CHỜ SERVER`, and avoid presenting extrapolation as confirmed position.
7. The window is transparent, click-through, follows the game client rectangle, remains topmost only while appropriate, and hides when the game is not foreground—matching the existing HUD safety behaviour.

This is a 2D course-relative aid. Without camera telemetry it cannot make a geometrically truthful world-space laser appear to touch the destination in the 3D scene.

## 7. Architecture

### Rust backend

Add a `water_guide` module responsible for:

- decoding and validating the freshwater mask;
- building/caching boundary candidates;
- selecting and locking `A` and `B`;
- holding enabled/disabled/error state behind application state synchronization;
- returning destination metadata without exposing cookies, tokens or private URLs;
- emitting state-change events and handling `toggle_water_guide`.

Extend hotkey dispatch and settings with default `Ctrl+Alt+W`. Existing conflict validation remains authoritative.

Extend the existing overlay-window supervisor patterns so the new window tracks the game client rectangle, foreground state, click-through state and lifecycle without restarting or injecting into The Isle.

### Frontend

Add a pure navigation module, proposed as `src/lib/navigation/water-guide.ts`, for:

- segment projection;
- cross-track error;
- look-ahead selection;
- shortest signed angle;
- U-turn and stale-data state derivation.

Add a dedicated transparent entrypoint under `src/water-guide/` with its own markup, TypeScript and CSS. Render at the existing position snapshot cadence, with animation interpolation up to the display frame rate. Interpolation improves visual smoothness but never manufactures server-confirmed positions.

## 8. User-visible states

- `ĐANG TÌM NƯỚC...`: validating data and selecting a target.
- `NƯỚC: <label> · <distance>`: active route.
- `LỆCH ĐƯỜNG <distance> · THEO TIA XANH`: catch-up guidance.
- `QUAY ĐẦU`: facing more than 110 degrees away from the steering target.
- `CHỜ SERVER`: position confirmation is stale.
- `XOAY / ĐI VÀI BƯỚC ĐỂ XÁC ĐỊNH HƯỚNG`: insufficient course evidence.
- `ĐÃ TỚI NGUỒN NƯỚC`: within the arrival radius.
- `KHÔNG XÁC MINH ĐƯỢC NƯỚC UỐNG`: fail-closed data error, with no ray drawn.

## 9. Error handling and observability

- Log only map/version, asset hashes, dimensions, candidate counts, selected POI label, approximate distances and state transitions.
- Never log Steam tokens, cookies, session URLs or full authentication payloads.
- Surface a concise actionable error in the Overlay and retain detailed non-secret diagnostics locally.
- One bad asset or hotkey conflict must disable Water Guide only; it must not take down the existing HUD, Night Vision, minimap or full map.

## 10. Test strategy

### Unit and regression tests

- Synthetic RGBA masks prove transparent ocean is excluded.
- Only shallow boundary pixels are eligible; deep interior centres are not selected.
- Inward offset remains within freshwater.
- Nearest valid destination is deterministic.
- Target `B` remains locked until off/on.
- Segment projection, cross-track error and 80 m look-ahead cover endpoints and degenerate segments.
- Shortest-angle tests cover `0°`, left/right turns, wraparound, and exact/near `180°`.
- U-turn and arrow direction cannot point back toward the ray origin.
- Missing/stale position or heading never emits a fabricated confident direction.
- Default hotkey is `Ctrl+Alt+W`; duplicate/conflicting bindings fail clearly.

### Integration and safety tests

- Validate current installed mask and POI dataset, recording dimensions, counts and hashes.
- Verify the Water Guide window follows the game client and hides on Alt-Tab.
- Verify click-through behaviour and that disabling it removes the window.
- Scan source and packaged artifacts for forbidden game-memory, injection, packet-capture and synthetic-input APIs.
- Run the existing navigation, frontend type-check and Rust workspace suites to prevent regression.

## 11. Delivery and live acceptance

1. Build and package from the isolated branch without touching the clean Night Vision branch.
2. Install/restart only the Overlay where practical; preserve the running game and user data.
3. Launch or attach to The Isle and capture visible proof of the ray over the actual game window.
4. Compare the locked destination against the same freshwater mask/full-map location and record the chosen label, coordinates, distance and data freshness.
5. Confirm `Ctrl+Alt+W` toggles on and off, the ray responds to a real direction change, U-turn is understandable, stale data is honest, and Alt-Tab hides it.
6. Actual walking to the destination and drinking remains a user-controlled live acceptance gate. The assistant may navigate menus for setup/testing, but must not synthesize character movement or bypass anti-cheat boundaries.
7. Do not merge or push a public release until tests pass and the owner accepts the live visual result.

## 12. Git isolation

- Worktree: `.worktrees/water-guide-ray`
- Branch: `codex/water-guide-ray-v1`
- Dedicated Cargo target: `D:\CodexBuild\theisle-overlay-water-guide-target`
- Existing dirty/user branches are not reset, cleaned, stashed or overwritten.

## 13. Acceptance criteria

The feature is acceptable only when all of the following are true:

- `Ctrl+Alt+W` reliably toggles Water Guide without a hotkey conflict.
- The chosen destination is demonstrably inside the freshwater mask near a shore boundary, never ocean or arbitrary deep water.
- Route endpoints remain fixed for the activation session.
- A new player can identify forward direction, a wrong-way U-turn and off-route recovery without opening the minimap.
- Stale/unknown telemetry is visibly distinguished from confirmed guidance.
- Existing Overlay features continue to pass their tests.
- No forbidden game integration or automated character control is introduced.
- Live game-window evidence is captured; a green build alone is not treated as completion.
