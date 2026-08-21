//! The one-way position flow: (clipboard | replay | debug) -> tracker -> both
//! windows. Port of the sample-handling wiring from the original `main.py`.

use overlay_core::{bearing_to_compass_key, world_to_pixel, Calibration};
use tauri::{AppHandle, Manager};

use crate::events::{
    emit_to_visible, PositionUpdate, TrailPayload, POSITION_UPDATE, SETTINGS_CHANGED,
    TRAIL_CHANGED,
};
use crate::state::{AppState, LockExt};

/// Feed one accepted coordinate sample through the tracker and notify the UI.
pub fn ingest_sample(app: &AppHandle, x: f64, y: f64, z: f64) {
    let state = app.state::<AppState>();
    let now_s = state.now_s();
    let cal = Calibration::gateway();

    let (outcome, heading, trail) = {
        let mut tracker = state.tracker.lock_safe();
        let outcome = tracker.add_sample(x, y, z, now_s);
        let heading = tracker.heading(now_s);
        let trail = outcome
            .trail_changed
            .then(|| trail_payload(&tracker.segments, cal));
        (outcome, heading, trail)
    };

    // Persist AFTER releasing no locks out of order: trail writes follow the
    // same order the original used — break record first, then the sample.
    if !outcome.refreshed_only {
        if let Some(writer) = state.trail_writer.lock_safe().as_mut() {
            if outcome.broke_segment {
                writer.add_break();
            }
            writer.add(x, y, z);
        }
    }

    let (px, py) = world_to_pixel(x, y, cal);
    let payload = PositionUpdate {
        x_cm: x,
        y_cm: y,
        z_cm: z,
        px,
        py,
        heading_deg: heading,
        compass_key: heading.map(bearing_to_compass_key),
        in_bounds: overlay_core::is_in_bounds(px, py, cal),
    };
    emit_to_visible(app, POSITION_UPDATE, payload);
    if let Some(trail) = trail {
        emit_to_visible(app, TRAIL_CHANGED, trail);
    }
}

/// The current tracker state as a PositionUpdate, or None before the first
/// sample. Shared by `resync` and the `get_current_position` command so a
/// freshly (re)loaded webview paints at once instead of waiting for the
/// player's next manual coordinate copy.
pub fn current_payload(state: &AppState) -> Option<PositionUpdate> {
    let now_s = state.now_s();
    let cal = Calibration::gateway();
    let (current, heading) = {
        let tracker = state.tracker.lock_safe();
        (tracker.current, tracker.heading(now_s))
    };
    let cur = current?;
    let (px, py) = world_to_pixel(cur.x, cur.y, cal);
    Some(PositionUpdate {
        x_cm: cur.x,
        y_cm: cur.y,
        z_cm: cur.z,
        px,
        py,
        heading_deg: heading,
        compass_key: heading.map(bearing_to_compass_key),
        in_bounds: overlay_core::is_in_bounds(px, py, cal),
    })
}

/// Bring a window that was hidden (and therefore skipped by
/// `emit_to_visible`) back up to date. Called right after showing it.
pub fn resync(app: &AppHandle) {
    let state = app.state::<AppState>();
    let cal = Calibration::gateway();

    let trail = {
        let tracker = state.tracker.lock_safe();
        trail_payload(&tracker.segments, cal)
    };
    if let Some(payload) = current_payload(&state) {
        emit_to_visible(app, POSITION_UPDATE, payload);
    }
    emit_to_visible(app, TRAIL_CHANGED, trail);
    {
        let settings = state.settings.lock_safe().clone();
        emit_to_visible(app, SETTINGS_CHANGED, settings);
    }
    emit_to_visible(app, "waypoints://changed", ());
    crate::islepilot::emit_last(app);
}

pub fn trail_payload(segments_cm: &[Vec<(f64, f64)>], cal: &Calibration) -> TrailPayload {
    TrailPayload {
        segments_cm: segments_cm.to_vec(),
        segments_px: segments_cm
            .iter()
            .map(|seg| {
                seg.iter()
                    .map(|&(x, y)| world_to_pixel(x, y, cal))
                    .collect()
            })
            .collect(),
    }
}
