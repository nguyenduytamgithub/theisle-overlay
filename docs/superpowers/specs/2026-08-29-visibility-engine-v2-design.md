# Visibility Engine v2 Design

**Status:** Approved by the user's explicit instruction on 2026-08-29 to build
the strongest practical client-only visibility mode immediately and continue
until a real night-scene result is proven.

**Target release:** v1.8.0

**Supersedes:** The v1.7.4 Magnification-only Night Vision implementation as the
primary renderer. Magnification remains a truthful compatibility fallback.

## Goal and truth boundary

Make The Isle materially easier to navigate at night from the pixels already
drawn on this PC. The result must outperform the game's `X` mode alone in a
paired live-night comparison while keeping silhouettes, terrain edges, and
bright highlights readable.

The engine cannot reconstruct geometry, textures, or distant objects that the
game did not render. It therefore must not promise literal daylight under all
fog, weather, or server conditions. Completion means a visually and
measurably clearer client-side image than `X` alone, not a renamed brightness
slider or a green process check.

## Why v1.7.4 is insufficient

The current Magnification adapter applies one 5x5 color matrix with a diagonal
RGB gain of 1.0x to 5.0x. A matrix can scale and translate colors, but it cannot
measure the scene, apply a nonlinear shadow curve, compare neighboring pixels,
or sharpen local edges. This explains why the game's `X` mode, which has scene
and depth knowledge inside the engine, can reveal more useful structure.

Microsoft recommends Windows Graphics Capture or Desktop Duplication for modern
screen capture. Windows Graphics Capture can target the game's HWND directly,
and a Direct3D11 frame pool exposes the captured BGRA surface without a CPU
round trip. That is the selected primary path. Desktop Duplication is reserved
for diagnostics, not the default, because a whole-monitor capture is more
likely to recurse through Overlay windows.

## Selected architecture

### 1. Window-scoped GPU capture

- Locate the visible The Isle top-level HWND through the existing process and
  foreground supervisor.
- Create a `GraphicsCaptureItem` for that exact HWND through
  `IGraphicsCaptureItemInterop::CreateForWindow`.
- Receive frames from a free-threaded `Direct3D11CaptureFramePool` in
  `B8G8R8A8UIntNormalized` format.
- Recreate the frame pool after a size or display-mode change and stop it on
  OFF, Alt-Tab, game minimize/exit, or device loss.
- Never open the game process, inject a DLL, hook DirectX, inspect memory,
  synthesize input, inspect network traffic, save frames, or transmit pixels.

Window capture is expected to work in Windowed/Borderless mode. Exclusive
Fullscreen, protected content, an unsupported Windows capture session, or a
driver/device failure must switch to the labeled Magnifier fallback rather than
claiming that the adaptive GPU renderer is active.

### 2. GPU visibility pipeline

The captured texture stays on the RTX 3060 and passes through two D3D11 stages:

1. **Luminance probe:** downsample the scene to a tiny luminance target. Read
   one aggregate value no more than four times per second and smooth it with an
   asymmetric exponential filter: brighten quickly when darkness arrives and
   reduce exposure slowly when highlights appear. This avoids ten-second jumps
   and visible pumping.
2. **Visibility shader:** apply exposure, a nonlinear shadow lift, highlight
   compression, desaturation protection, and a small five-tap local-contrast
   enhancement. The shader uses the smoothed luminance plus the selected
   preset; it never invents positions or game state.

The tone curve is deterministic and mirrored by a pure Rust reference function
for unit and image-fixture tests. Parameters are bounded to avoid NaN, negative
color, total whiteout, or unstable frame-to-frame gain.

### 3. Native presentation overlay

- Present the processed texture through a borderless Direct3D11/DXGI surface
  owned by a transparent, non-activating, click-through top-level window.
- Keep the processed-image window directly above the game and below the HUD,
  minimap, compass, and Night Vision button.
- Match the game's client rectangle and DPI every supervision tick.
- Use a two-buffer flip-model swap chain and never block the game render thread.
- If the newest capture is late, retain the last processed frame; do not stall
  input or spin at 100% CPU.

The image window contains only the transformed game image. Because capture is
scoped to the game HWND, Overlay UI is not fed back into the effect.

### 4. Presets and control

The existing on-screen `NHÌN ĐÊM` button and `Ctrl+Alt+N` remain the primary
toggle. The settings expose three honest presets:

- **Cân bằng:** moderate shadow lift and sharpening with the lowest GPU cost.
- **Rõ:** stronger adaptive exposure for normal play.
- **Cực sáng:** visibility-first limits for the darkest weather; this may look
  less natural and is the default requested mode for acceptance testing.

`Tự động` uses the luminance probe. `Ép sáng` keeps the chosen preset active
even when the scene is not classified as dark. The button status must identify
`GPU thích nghi`, `Dự phòng Magnifier`, `Đang chờ game`, or `Lỗi`; it may report
ON only after renderer readback confirms current HWND, rectangle, preset, and a
recent presented frame.

The game `X` mode remains independent. The recommended strongest combination
is `X` ON plus Overlay `Cực sáng`; the Overlay neither presses `X` nor reads its
state.

### 5. Compatibility fallback

The existing Magnifier adapter remains audited and receives a stronger bounded
color matrix with both gain and black translation. It is a fallback for capture
or device failure, not the adaptive implementation. UI and logs must clearly
say which renderer is active.

Fallback success requires native matrix/source readback. It must never be
represented as local-contrast enhancement or GPU-adaptive processing.

## Performance and recovery

- Primary target: 60 presented frames per second at 1920x1080.
- Acceptance ceiling: 33 ms median present interval and no sustained Overlay
  CPU spin. If the rolling interval exceeds the ceiling, reduce the visibility
  overlay to 30 FPS before weakening the game's own performance.
- Luminance readback is rate-limited to 4 Hz; all full-resolution work remains
  GPU-side.
- Recover once from DXGI device removal, capture access loss, window recreation,
  resize, and sleep/display changes. Repeated failure enters fallback with the
  exact reason logged.
- Preserve v1.7.4 settings, login token, waypoints, trails, and navigation HUD.

## Safety and anti-cheat disclosure

This feature captures and redraws pixels displayed by the game in a separate
client process. It does not modify the server, weather, day/night clock, game
files, engine, process memory, or packets. It does not attempt to bypass,
conceal itself from, disable, or interfere with Easy Anti-Cheat.

The project can verify its own technical boundary but cannot claim that every
community server or anti-cheat operator has approved third-party overlays.
Documentation must retain that warning and provide an immediate OFF/restore
path.

## Test strategy

### TDD and deterministic fixtures

- Start with failing tests for the luminance smoothing, preset bounds, tone
  curve, highlight protection, local-contrast kernel, state transitions,
  fallback truth labels, and device-loss recovery.
- Add fixed dark, mixed, and bright image fixtures. The reference processor
  must increase dark-detail separation without flattening a bright fixture.
- Compile the HLSL shader during tests and compare a small GPU render against
  the reference implementation within an explicit quantization tolerance.
- Extend the forbidden-API gate to reject process/memory access, injection,
  graphics hooks, input synthesis, networking, frame persistence, and capture
  of any target other than the selected game HWND.

### Full automated gate

- Rust unit, integration, safety, release, and renderer tests.
- `cargo fmt --check` and Clippy with warnings denied.
- Svelte diagnostics with 0 errors and 0 warnings.
- Production frontend and NSIS build.
- Fresh CodeGraph synchronization and focused call-path audit after changes.

### Installed live-night gate

Use one unchanged nighttime view and capture three paired samples:

1. Overlay OFF, game `X` OFF;
2. Overlay OFF, game `X` ON;
3. Overlay `Cực sáng` GPU mode ON, game `X` ON.

For a HUD-free play-area region, sample 3 must meet all of these:

- median luminance at least 1.6x sample 2, or reach 0.20 normalized luminance;
- dark-region local contrast at least 1.20x sample 2;
- clipped-pixel ratio below 15%;
- median presentation interval at or below 33 ms; and
- terrain, foliage, and the dinosaur silhouette visibly easier to distinguish
  than in sample 2.

Also verify click-through input, z-order, resize, Alt-Tab hide/restore, OFF
cleanup, no stale capture session, current renderer label, game PID continuity,
and preserved user data hashes.

The comparison images and metrics are release evidence, but final acceptance
still requires the user to inspect the installed result in the live game.

## Delivery order

1. Add reference tone mapping, fixtures, state model, and the improved truthful
   Magnifier fallback.
2. Add the Windows Graphics Capture/D3D11 renderer and native presentation.
3. Wire the existing UI, settings migration, diagnostics, and recovery.
4. Build a v1.8.0 candidate to `D:\CodexBuild`, install it without stopping The
   Isle, and run the live-night gate.
5. Tune only bounded preset parameters until the paired proof passes. Push the
   public branch and update the pull request after technical and human PASS.

## Official API basis

- Windows Graphics Capture overview:
  <https://learn.microsoft.com/windows/apps/develop/media-authoring-processing/screen-capture>
- Targeting one HWND with `CreateForWindow`:
  <https://learn.microsoft.com/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createforwindow>
- Desktop Duplication GPU surfaces and frame updates:
  <https://learn.microsoft.com/windows/win32/direct3ddxgi/desktop-dup-api>
- Magnification color-matrix limitation:
  <https://learn.microsoft.com/windows/win32/api/magnification/ns-magnification-magcoloreffect>
