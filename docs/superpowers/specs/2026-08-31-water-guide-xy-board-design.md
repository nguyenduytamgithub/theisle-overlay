# Water Guide North-Up XY Board Design

## Goal

Replace the camera-glued centre ray with a large, north-up navigation board drawn directly over the game window. The board must guide a new player toward the locked freshwater target using confirmed XY coordinates only and must never react to mouse or camera rotation.

## Accepted interaction

- `Ctrl+Alt+W` toggles Water Guide.
- The board is a dedicated full-game overlay, not content inside the minimap or big map.
- While Water Guide is on, the ordinary heading HUD and rotating minimap remain suppressed so no camera-relative arrow competes with the board.
- Night Vision remains independently usable through `Ctrl+Alt+N`.
- Toggling Water Guide off restores the ordinary HUD/minimap.

## Visual design

The old vertical ray and its chevrons are removed. A translucent navigation board is placed in the lower-middle part of the game window and contains:

1. A fixed compass ring labelled `BẮC`, `ĐÔNG`, `NAM`, and `TÂY`. North is always the top of the board.
2. A cyan target needle from the centre toward the absolute XY bearing of the locked freshwater target.
3. A white movement needle derived only from confirmed XY displacement of at least five metres. It turns green when aligned with the target needle.
4. A destination row with freshwater label and remaining distance.
5. Large Vietnamese steering copy: `ĐI VÀI BƯỚC ĐỂ LẤY HƯỚNG`, `RẼ TRÁI N°`, `RẼ PHẢI N°`, `QUAY ĐẦU N°`, `ĐÚNG HƯỚNG · GIỮ W`, `CHỜ TỌA ĐỘ MỚI`, or `ĐÃ TỚI NGUỒN NƯỚC`.

The board has no CSS animation, rotation transition, pulsing, drifting chevrons, or view-relative movement. Needle angles may change only after accepted XY evidence changes.

## Geometry and data flow

- Current and target XY produce `targetBearingDeg` in the existing map calibration: north is 0 degrees, east 90, south 180, and west 270.
- Confirmed displacement of at least 500 cm produces `movementCourseDeg`; smaller displacement is treated as jitter and does not move the movement needle.
- `relativeDeg = shortestSigned(targetBearingDeg - movementCourseDeg)` drives the steering text and alignment hysteresis.
- The target needle uses absolute `targetBearingDeg`. The movement needle uses absolute `movementCourseDeg`. Neither uses `serverFacing`, character head direction, camera yaw/pitch/FOV, view matrices, mouse input, frame capture, or the normal NavigationEstimator.
- A new accepted XY sample recomputes current-to-target bearing and remaining distance. Camera-only movement cannot emit a position event and therefore cannot change either needle.

## State handling

- No route or invalid freshwater data: hide the board needles and show the existing fail-closed error copy.
- No movement course yet: show the target needle, hide the movement needle, and instruct the player to walk at least five metres.
- Stale/invalid position quality: freeze the last target needle, dim the movement needle, and show `CHỜ TỌA ĐỘ MỚI`.
- Within 25 metres: hide both needles and display arrival.
- Toggling to a new route resets movement evidence and alignment state.

## Safety boundary

The feature is player-controlled guidance only. It does not read game memory, hook or inject into the game, capture packets, inspect continuous game frames, synthesize game input, control the dinosaur, or avoid obstacles. It consumes only the Overlay's existing confirmed position events and verified freshwater target.

## Testing and acceptance

- Pure tests prove cardinal target bearings, shortest left/right/U-turn instructions, five-metre jitter rejection, arrival, waiting, and no movement-course state.
- Source-contract tests prove the renderer contains no camera/head/server-facing inputs and the CSS contains no animation.
- Regression tests prove the Water Guide still suppresses the ordinary HUD/minimap only while requested.
- Frontend checks, the full navigation test suite, Vite build, Rust library tests, and clippy must pass.
- Live acceptance requires two desktop captures with clearly different game camera angles while the same confirmed XY state keeps the board needles at identical screen angles. Build/install evidence alone is not acceptance.

## Rejected approaches

- A screen-centred ray was rejected because it is glued to the camera view and falsely appears to mark a world path.
- A true ground-anchored 3D beam was rejected because the Overlay has no legitimate camera pose/FOV/view-matrix source; XY endpoints alone cannot be projected into the current view.
- Server-facing or character-head direction was rejected because it is not camera state and was observed to jump.
