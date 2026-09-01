# Waypoint XY Board Design

## Goal

Make the existing `➤ Chỉ hướng` action for a saved waypoint open the same north-up XY board that Water Guide uses, directly over the game. The waypoint becomes the destination instead of the nearest freshwater source, and the shared visual is made compact enough not to hide important gameplay detail.

## User interaction

- Clicking `➤` beside a waypoint immediately selects that waypoint and activates the full-game XY board.
- Clicking `■` stops waypoint guidance and hides the board.
- `Ctrl+Alt+W` still starts or stops freshwater guidance.
- The two destination sources are exclusive and last-action-wins:
  - selecting a waypoint turns freshwater guidance off;
  - activating freshwater guidance clears the selected waypoint.
- The cyan needle is the absolute current-XY-to-destination bearing.
- The white needle is the confirmed movement course after at least 500 cm of travel and becomes green when aligned.
- Mouse movement, camera angle, character head/facing, and screen capture never enter either needle calculation.

## Compact presentation

Both waypoint and freshwater modes use the same compact renderer so their behavior cannot drift.

- The compass disc is responsive and capped at 230 px on a 1920 x 1080 display.
- It stays at the lower middle of the game window, above the bottom edge and away from the top status area.
- The dark panel remains translucent and click-through.
- Destination, distance, and maneuver are shown in one compact status pill instead of a second large duplicate yellow banner.
- Full Vietnamese cardinal labels remain visible: `BẮC`, `ĐÔNG`, `NAM`, `TÂY`.
- Waypoint copy uses `ĐIỂM: <name> · <distance>`; freshwater copy keeps `NƯỚC: <name> · <distance>`.

## Architecture

The existing `water-guide` webview becomes a shared XY guidance renderer without changing its anti-cheat-safe process boundary. It listens to the existing `navigation://changed`, `waypoints://changed`, `settings://changed`, and position events in addition to `water-guide://changed`.

The backend window supervisor treats either a requested Water Guide route or a selected waypoint ID as a request to show the shared overlay. The frontend converts the selected `NavigationTarget` into the existing `WaterGuideRoute` geometry contract and keeps its start point stable until the waypoint changes. Existing position freshness, movement threshold, alignment hysteresis, arrival, and non-animated needle rules are reused.

The normal heading HUD and rotating minimap are suppressed while either source owns the shared XY board. They return when both sources are off.

## Source exclusivity

Backend commands enforce one destination source:

1. `set_navigation_target(Some(id))` deactivates an active freshwater request before broadcasting navigation changes.
2. Turning Water Guide on clears `navigation.target_waypoint_id` and broadcasts the existing settings/navigation events before publishing the freshwater state.
3. Stopping either source does not activate the other source automatically.

This produces deterministic behavior after restart and avoids two frontends guessing which event happened last.

## Failure and stale behavior

- A selected waypoint with no confirmed position shows a compact waiting message and no fabricated needle.
- Invalid/deleted waypoint IDs fail closed and hide the shared board after the existing target setting is cleared.
- Stale or rejected coordinates freeze the last known needles and show the waiting state.
- Arrival uses the existing navigation arrival threshold; the board reports arrival and hides its needles.
- Initial command snapshots cannot overwrite newer navigation, Water Guide, position, or quality events.

## Verification

- Pure tests cover waypoint-to-route conversion, source selection, target changes, arrival, and waiting behavior.
- Renderer contract tests prove the shared frontend listens to navigation events, uses the same XY geometry, contains no camera/mouse source, and uses the compact size/status pill.
- Rust tests cover combined window visibility and source exclusivity.
- Full Node, Svelte, Vite, Rust, Clippy, diff, and prohibited-source gates must pass.
- Installation restarts only Overlay. Live acceptance requires the compact board to appear directly over the game after `➤`, disappear after `■`, and remain readable without covering a large part of the screen.

## Safety boundary

No game-memory access, DLL or graphics injection/hooks, packet capture, low-level keyboard hooks, continuous game capture, synthetic game input, character control, terrain avoidance, or anti-cheat bypass is added.
