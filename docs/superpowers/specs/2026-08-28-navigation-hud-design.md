# Reliable Navigation and In-Game Compass Design

## Goal

Make the overlay useful during normal play despite IslePilot's irregular 5-10 second position updates: reject impossible jumps, prefer the server's authoritative yaw for direction, render honest short-lived motion prediction, let the user select a waypoint as the navigation target, and show a click-through north-up compass HUD over the game.

## Evidence and root cause

- The saved 2026-08-27 trail contains 348 samples, 13 steps above 20 m/s, and eight steps above 200 m.
- One observed sequence jumped about 7.9 km in three seconds and returned about 7.9 km six seconds later.
- `PositionTracker::add_sample` currently makes every non-duplicate sample current. A large jump breaks the trail, but the position event and heading still use the bad point.
- Heading currently comes from only the last two accepted coordinates, requires 20 m of movement, and remains valid for ten minutes.
- Both IslePilot token and marker payloads already include yaw, but the production ingestion path drops it.
- Waypoint navigation currently always chooses the nearest saved waypoint. Selecting a waypoint in the list only centers the full map.
- The minimap has a north-up compass, but there is no independent compass/navigation HUD on the game screen.
- The game's local log does not publish live movement coordinates or yaw. Process-memory reading, injection, or anti-cheat bypass is out of scope.

## Product behavior

### Confirmed samples and outliers

All automatic and clipboard coordinates pass through one tracker. Inputs must be finite. A candidate movement is plausible when its distance is within a fixed uncertainty allowance plus a maximum speed multiplied by elapsed time. A single implausible sample is quarantined: it does not update the current marker, heading, trail, persistence, or UI events.

If the next sample is close to the quarantined location and plausible relative to it, the pair confirms a real relocation or respawn. The tracker accepts the newest sample, starts a disjoint trail segment, clears velocity, and never draws a line from the old location. If the next sample instead returns near the last confirmed location, the quarantined spike is discarded.

Normal long-distance samples that are plausible for their elapsed time remain in one segment; the old fixed 200 m rule must not break a trail merely because an update was delayed. A time gap beyond the configured session break still starts a new segment.

### Heading

`pipeline::ingest_sample` accepts an optional compass bearing. Token API and marker JSON Unreal yaw are converted to compass bearing with `bearing = (180 - yaw) mod 360`; the HTML map transform is already a north-up compass bearing and is passed through directly.

A fresh authoritative server bearing wins. When it is absent, heading is derived from a short window of recent accepted coordinates rather than one noisy pair. Relocations and rejected samples never produce a heading. The UI rotates through the shortest angular path.

### Honest visual prediction

The backend exposes confirmed position, received time, velocity in world and active-map pixels, heading source, and freshness metadata. Frontends may extrapolate the visual marker for no more than four seconds. The predicted marker/segment is visually distinct from confirmed trail data and is never persisted.

When a new confirmation arrives, the visual marker reconciles smoothly over less than one second. Prediction stops when velocity is unknown or data is stale. After twelve seconds without a fresh confirmation, the HUD shows `DỮ LIỆU CŨ` and does not continue inventing movement. This is a display aid, not a claim of true local telemetry.

The IslePilot poll setting is automatically lowered from 10 seconds to 5 seconds for this build. Duplicate server responses are deduplicated by unchanged position/yaw so faster polling cannot add duplicate trail points.

### Selected waypoint navigation

Exactly one saved waypoint may be the active target. Target identity is stored in settings so the minimap, full map, HUD, and next launch agree. Deleting the active waypoint clears the target.

The full map highlights the target and draws a straight north-up line from the latest confirmed position to it. The minimap and HUD show its absolute bearing, distance, and relative turn (`trái`, `phải`, or `thẳng`) using the latest reliable heading. Arrival is reported at 15 m or less. This is direct guidance only; it does not claim terrain-aware routing.

### Game-screen HUD

A separate minimal Tauri webview named `navigation-hud` is transparent, click-through, always on top, absent from the taskbar, and anchored to the top-center of The Isle client area. It is shown only while the game is foreground and the main overlay window is not in front.

The north-up compass rose always shows the full Vietnamese words `BẮC`, `ĐÔNG`, `NAM`, and `TÂY`. A yellow arrow shows player heading; a blue arrow shows target bearing. Text shows current compass direction/degrees, target turn/distance, and whether position is confirmed, predicted, or stale. `Ctrl+Alt+H` toggles the HUD without affecting the minimap.

## Architecture

- `overlay-core::tracker` owns sample acceptance, quarantine/relocation confirmation, velocity, heading precedence, and trail segmentation. It remains pure Rust and receives deterministic timestamps for tests.
- IslePilot parsers preserve yaw and convert it at the adapter boundary before calling the shared pipeline.
- `PositionUpdate` adds motion/freshness fields; a new navigation command/event exposes the selected waypoint guidance.
- `src/lib/navigation.ts` owns pure visual prediction, angle interpolation, and relative-turn calculations shared by the minimap, full map, and HUD.
- The new HUD has its own HTML/Vite entry and lightweight canvas renderer. Rust window supervision mirrors the minimap's foreground, topmost, click-through, and anchoring safety.

## Safety and compatibility

- Do not read game memory, inject DLLs, synthesize game input, inspect anti-cheat internals, or modify The Isle files.
- Do not expose IslePilot cookies or overlay tokens in logs or tests.
- Existing settings, waypoints, trail files, clipboard coordinate parsing, basemap transforms, and hotkeys remain compatible.
- Existing stored trails are not rewritten. Only newly ingested samples use the improved filter.
- Automatic updater installation remains user-triggered. The custom build uses a prerelease version so it is visibly distinguishable.

## Verification

- Rust regression tests reproduce the 7.9 km spike-return sequence, plausible delayed movement, confirmed relocation, duplicate sample, stale heading, yaw conversion, angle wrap, and trail segmentation.
- TypeScript tests cover four-second prediction cap, stale freeze, shortest-angle interpolation, all waypoint quadrants, relative turn, and arrival radius.
- Existing Rust workspace tests, frontend checks, production frontend build, and Tauri release bundle must pass.
- Runtime QA without game control verifies window creation, transparent/click-through flags, event payloads, and installer upgrade preservation. Final in-game directional verification is requested from the user only after automated checks pass.
