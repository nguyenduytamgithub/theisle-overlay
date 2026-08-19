//! The one-way position flow: (clipboard | replay | debug) -> tracker -> both
//! windows. Port of the sample-handling wiring from the original `main.py`.

use overlay_core::{bearing_to_compass_key, world_to_pixel, Calibration};
use tauri::{AppHandle, Emitter, Manager};

use crate::events::{PositionUpdate, TrailPayload, POSITION_UPDATE, TRAIL_CHANGED};
use crate::state::AppState;

/// Feed one accepted coordinate sample through the tracker and notify the UI.
pub fn ingest_sample(app: &AppHandle, x: f64, y: f64, z: f64) {
    let state = app.state::<AppState>();
    let now_s = state.now_s();
    let cal = Calibration::gateway();

    let (outcome, heading, trail) = {
        let mut tracker = state.tracker.lock().unwrap();
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
        if let Some(writer) = state.trail_writer.lock().unwrap().as_mut() {
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
    if let Err(e) = app.emit(POSITION_UPDATE, payload) {
        log::warn!("emit position failed: {e}");
    }
    if let Some(trail) = trail {
        if let Err(e) = app.emit(TRAIL_CHANGED, trail) {
            log::warn!("emit trail failed: {e}");
        }
    }
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
