# Fixed-Center Water Guide Design

## Problem

The current Water Guide rotates its full-screen ray by the relative destination bearing. Moving the mouse therefore moves the ray, while the UI does not provide an unmistakable moment at which a new player should stop turning.

## Approved behavior

- The main ray is always vertical and fixed at the horizontal center of the game screen.
- A separate large maneuver prompt reports `XOAY TRÁI`, `XOAY PHẢI`, or `QUAY ĐẦU`, including the rounded angular error.
- Alignment locks when the absolute angular error is at most 8 degrees.
- A locked alignment remains locked until the error exceeds 18 degrees, preventing flicker from noisy server headings.
- While locked, the ray and maneuver prompt turn green and display `ĐÚNG HƯỚNG · GIỮ W`.
- Waiting, stale, arrived, and invalid states continue to hide the ray and fail closed.
- `Ctrl+Alt+W` continues to select and lock the nearest verified freshwater route; `Ctrl+Alt+N` and Night Vision behavior remain unchanged.

## Architecture

`src/lib/navigation/water-guide.ts` owns the fixed-ray contract, alignment hysteresis, and localized maneuver text as pure functions. `src/water-guide/main.ts` keeps only the previous alignment bit between paint frames and renders the pure result. `water-guide.html` and `src/water-guide/style.css` provide a general maneuver banner and the green locked state.

The route, target selection, server-position estimator, and EAC-safe boundary do not change. The fixed center ray represents the character's forward direction; the maneuver prompt tells the player how to rotate that forward direction toward the water target.

## Acceptance

- Turning the mouse never rotates the center ray.
- At 8 degrees error the UI becomes locked and green.
- Between 8 and 18 degrees an existing lock remains stable.
- Above 18 degrees the UI unlocks and gives an explicit left/right instruction.
- Navigation, Rust, Svelte, Vite, Clippy, installer, and live Night Vision coexistence checks pass.

## XY-only correction (final owner decision)

- Water Guide must never consume server facing, head direction, or camera/mouse rotation.
- The ray is pinned vertically to the screen center. Mouse movement cannot move or rotate it.
- Every accepted server coordinate recomputes the direct vector from the latest confirmed `(xCm, yCm)` to the locked freshwater target.
- Turn/alignment truth comes only from displacement between confirmed XY samples. A sample must move at least 1 metre from the retained anchor before it may define a movement course.
- Before a valid movement course exists, keep the ray visible and say to walk a few steps; do not claim a direction or green lock.
- When the coordinate becomes stale, freeze the last XY ray visibly and label it as waiting; do not hide, rotate, or advance it locally.
- Reset the XY course anchor on relocation, rejected position quality, route replacement, or guide deactivation.
- Left/right copy must identify `QUỸ ĐẠO XY`; it must not claim to read the character's head.
