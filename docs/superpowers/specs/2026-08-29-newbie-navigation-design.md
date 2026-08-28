# Newbie Navigation v1.7 Design

**Status:** Approved direction; written specification awaiting final user review  
**Date:** 2026-08-29  
**Target:** public `navigation-hud` fork  
**Product goal:** A first-time The Isle player can select a waypoint, understand the direction to travel at a glance, and keep making progress despite sparse server updates, without an arrow that spins or falsely claims exact realtime position.

## 1. Evidence and problem statement

The installed v1.6.0 polls every five seconds, but the observed accepted trail has a median interval of about 16 seconds between changed coordinates. Its frontend predicts position for four seconds and then stops. The HUD smooths position correction but consumes the newest server heading directly. The target arrow therefore mixes a locally smoothed position with a discrete, unfiltered facing angle.

This creates four user-visible failures:

1. Movement is fluid briefly, freezes, then jumps when the next server coordinate arrives.
2. A changing or noisy server yaw rotates the arrow immediately.
3. Left/right text can oscillate even while the destination itself has barely moved.
4. Near the destination, small coordinate changes produce a large bearing change and make the arrow spin.

The redesign treats server data as periodic correction evidence, not a 30 FPS animation source.

## 2. Research-backed principles

- Android distinguishes location bearing—the horizontal direction of travel—from device orientation. The product must likewise separate **course of travel**, **character facing**, and **bearing to target**: <https://developer.android.com/reference/android/location/Location.html>
- Mapbox Navigation exposes raw and enhanced/map-matched locations and may provide predicted key points to bridge frequency and precision problems. This fork has no route graph, so it will adopt the separation and confidence model without pretending to perform road matching: <https://docs.mapbox.com/android/navigation/guides/device-location/>
- Google notes that poor location signals require interpolation and can reduce route precision. Local prediction must therefore be bounded, decaying, and visibly labeled: <https://developers.google.com/maps/documentation/navigation/android-sdk/reference/com/google/android/libraries/navigation/Navigator>
- ROS defines angular updates using the shortest angular distance. Every visual angle interpolation must cross 0°/360° by the short arc: <https://docs.ros.org/en/iron/p/angles/generated/function_angles_8h_1a83debbedd331511aa914f393db576818.html>
- Turn-by-turn systems present a small maneuver vocabulary rather than demanding that users interpret continuously changing raw degrees. The HUD will use stable Vietnamese maneuvers and keep exact bearings secondary: <https://developers.google.com/maps/documentation/navigation/android-sdk/reference/com/google/android/libraries/mapsplatform/turnbyturn/model/Maneuver>

## 3. Chosen approach and rejected alternatives

### Chosen: local, confidence-aware stable-target guidance

The large blue destination arrow is north-up and represents only the absolute bearing from the locally estimated position to the selected waypoint. It does not rotate with raw character yaw. A separate compact current-course indicator is shown only when its confidence is sufficient. Relative guidance is derived from the stable current course, not blindly from the latest server yaw.

The UI paints at about 30 FPS from a deterministic local estimator. New server samples correct the estimator. Server silence gradually reduces confidence and prediction velocity instead of producing either an abrupt freeze or unlimited dead reckoning.

### Rejected: only smooth server yaw

This would make the old design look less jerky but preserve the wrong primary reference. Character facing can legitimately change while the desired destination direction remains constant.

### Rejected: long constant-velocity extrapolation

It looks realtime but becomes confidently wrong as soon as the player turns, stops, hits terrain, or changes speed. It is unsuitable for a newbie-facing product.

### Rejected: capture keyboard, mouse, screen, game memory, or network packets

Those approaches add calibration problems, privacy cost, and anti-cheat risk. This release remains passive: HTTPS/clipboard input only, with no injection, hooks, synthetic input, memory reading, packet capture, or continuous screen capture.

## 4. Architecture and component boundaries

### 4.1 Pure local navigation estimator

Create a focused TypeScript module under `src/lib/navigation/`. It owns:

- bounded position projection;
- correction blending;
- circular filtering and angular-rate limiting;
- course confidence;
- target bearing smoothing;
- maneuver hysteresis;
- freshness state.

It consumes immutable confirmed position events and the current target. It returns one presentation snapshot for a given timestamp. It does not access the DOM, Tauri, storage, or the network, so recorded samples and synthetic edge cases can test it deterministically.

### 4.2 Backend tracker remains the truth boundary

Rust continues to:

- validate finite coordinates;
- quarantine implausible jumps;
- segment confirmed trails;
- publish confirmed position, velocity, source heading, and timestamps.

Rust will additionally expose enough sample quality metadata for the frontend to distinguish server facing from motion course. Confirmed trail files continue to contain confirmed points only; locally predicted points are never persisted as fact.

### 4.3 HUD becomes presentation-only

`src/hud/main.ts` passes events into the estimator and paints its output. It no longer implements prediction, heading selection, or maneuver thresholds inline.

Full map and minimap consume the same estimator semantics:

- confirmed trail: solid;
- local prediction tail: short dashed segment;
- destination line: clear, straight, and explicitly direct-to-target;
- no claim that the straight line avoids cliffs, water, walls, or terrain.

Each webview owns a small estimator instance because Tauri webviews do not
share JavaScript memory. Given the same confirmed event and evaluation
timestamp, every instance must return the same snapshot.

## 5. Estimation model

### 5.1 Local refresh rate

The estimator is evaluated at approximately 30 FPS. This makes UI motion locally realtime without increasing server load. The server polling interval stays at five seconds because more requests do not force the remote service to publish fresh coordinates.

### 5.2 Bounded, decaying projection

For a confirmed velocity `v` and sample age `t`:

- 0–4 seconds: use normal constant-velocity projection;
- 4–12 seconds: smoothly decay velocity with a three-second time constant;
- after 12 seconds: stop advancing and hold the last bounded estimate.

The decaying phase bridges the observed sparse cadence while preventing a long straight-line overshoot. A stop or tiny displacement immediately collapses prediction velocity. Relocation and outlier events clear all prediction state.

### 5.3 Server correction

When a confirmed coordinate arrives:

- ordinary correction under 30 metres: ease over 650 ms;
- correction from 30 to 100 metres: ease over 300 ms;
- accepted relocation or correction above 100 metres: snap and clear local velocity.

The visual path never draws a connecting line across a relocation.

### 5.4 Stable course and facing

Maintain separate values:

- `targetBearing`: estimated position to waypoint, always absolute north-up;
- `motionCourse`: circularly weighted course from recent accepted movement;
- `serverFacing`: transformed server yaw, secondary only;
- `guidanceCourse`: motion course when moving with confidence, otherwise stable server facing, otherwise unknown.

Angles are filtered in circular space and advanced only by the shortest arc. A four-degree deadband suppresses jitter. Visual guidance is limited to 120 degrees per second. A source switch must remain valid for one second before replacing the active guidance course.

Raw yaw may update the small facing indicator but cannot directly rotate the large destination arrow.

## 6. Newbie HUD behavior

The top-center HUD has one visual priority:

1. **Large blue target arrow** — north-up absolute direction to the selected waypoint.
2. **Large instruction** — one stable phrase:
   - `ĐI THẲNG`
   - `CHẾCH TRÁI`
   - `CHẾCH PHẢI`
   - `RẼ TRÁI`
   - `RẼ PHẢI`
   - `QUAY LẠI`
   - `GIỮ HƯỚNG <CARDINAL>` when relative course is not trustworthy.
3. **Target detail** — waypoint name, cardinal bearing, degrees, and remaining distance.
4. **Freshness**:
   - `ĐANG BÁM`: confirmed sample age at most six seconds;
   - `ĐANG ƯỚC LƯỢNG`: six to twelve seconds;
   - `CHỜ SERVER`: older than twelve seconds;
   - `MẤT VỊ TRÍ`: no valid position.

Maneuver thresholds use hysteresis. A label must remain eligible for at least 600 ms before replacing the current label, except arrival and signal loss, which take effect immediately. This prevents text from flipping around a threshold.

The base thresholds are:

- absolute course error at most 12°: straight;
- above 12° through 35°: slight left/right;
- above 35° through 110°: left/right;
- above 110°: turn back.

An existing label keeps a four-degree margin across its boundary before a
neighboring label can become eligible.

Exact degrees remain visible but visually secondary. Technical labels such as `SERVER`, `ESTIMATE`, and `STALE` are replaced with plain Vietnamese by default.

## 7. Arrival and near-target stability

The default arrival radius becomes 25 metres. During the schema-v1 to
schema-v2 migration, an existing value of exactly 15 metres is treated as the
legacy default and becomes 25 metres; any other existing value is preserved.

Inside the arrival radius:

- freeze the last stable arrow;
- replace maneuver text with `ĐÃ TỚI KHU VỰC ĐÍCH`;
- stop showing left/right corrections;
- keep distance visible.

This avoids the undefined/unstable bearing that occurs when the estimated position is nearly identical to the waypoint.

## 8. Route honesty and recovery

The current data has no verified walkable terrain graph or navmesh. The release therefore provides reliable **direct-to-waypoint guidance**, not automatic terrain-safe pathfinding.

To help a newbie recover:

- show the confirmed breadcrumb trail;
- show the direct target line;
- show whether distance has decreased across confirmed samples;
- after three confirmed samples whose total progress toward the target is less
  than 10 metres, display `ĐANG ĐI XA ĐÍCH — KIỂM TRA BẢN ĐỒ`;
- one click/hotkey reopens the full map centered on player and target.

The product must not claim that a direct line is a safe traversable route.

## 9. Error handling and observability

Add privacy-safe diagnostic logs containing:

- sample age and accepted/quarantined/relocated state;
- heading source only, not account data;
- raw, filtered, and displayed angles;
- confidence/freshness transitions;
- correction distance;
- estimator reset reason.

Logs must not contain Steam tokens, cookies, private URLs, or exact credentials. Coordinate logging stays behind the existing local diagnostic boundary and is not added to telemetry.

HUD readiness is independently verified. A missing `hud://ready` handshake triggers one bounded recreation and a clear log entry, not an unbounded restart loop.

## 10. Testing and acceptance

### Pure estimator tests

- 359° → 1° takes the two-degree short arc and never spins backward.
- Alternating noisy yaw cannot move the large absolute target arrow.
- A 16-second server gap advances smoothly, decays, then holds by 12 seconds.
- A server correction remains continuous at frame boundaries.
- A relocation snaps without a connecting prediction segment.
- Course source switching observes the one-second stability gate.
- Maneuver labels do not flap around left/right/straight thresholds.
- Arrival within 25 metres freezes guidance and reports arrival.
- No-progress detection requires three confirmed samples.

### Integration tests

- the same confirmed event and timestamp produce equivalent estimator snapshots
  in HUD, minimap, and full map;
- predicted points never enter the persisted trail;
- legacy default arrival radius migrates to 25 metres while custom values survive;
- HUD readiness recovery is bounded.

### Release gates

- Node navigation tests, Svelte check, frontend build;
- Rust workspace tests and Clippy with warnings denied;
- forbidden-API and credential-shaped-secret scans;
- clean Tauri release build;
- installer hash and size recorded;
- installation over v1.6.0 preserves token, settings, waypoints, and confirmed trails;
- live game acceptance: select one waypoint, travel for at least three confirmed server updates, verify no full-spin arrow, understandable guidance, decreasing distance, bounded correction, and truthful freshness state.

Source/build tests do not substitute for the final live game acceptance.

## 11. Versioning and delivery

- Version: `1.7.0 Newbie Navigation`.
- Work continues on the public fork's `navigation-hud` branch.
- Documentation explains the estimator boundary and the lack of terrain-safe pathfinding.
- Build and local installation happen only after automated gates pass.
- GitHub push/release happens only after local installation and the live acceptance result are recorded honestly.
