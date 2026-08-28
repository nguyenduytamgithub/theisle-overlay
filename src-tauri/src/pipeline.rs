//! The one-way position flow: (clipboard | replay | debug) -> tracker -> both
//! windows. Port of the sample-handling wiring from the original `main.py`.

use overlay_core::{
    bearing_to_compass_key, world_to_pixel, Calibration, HeadingSource, Sample, SampleOutcome,
};
// Note: every px in these payloads is computed with state.active_calibration()
// at emit time — nothing px-shaped is cached, so a basemap switch only needs a
// resync to repaint everything in the new frame.
use tauri::{AppHandle, Manager};

use crate::events::{
    emit_all, PositionUpdate, TrailPayload, POSITION_UPDATE, SETTINGS_CHANGED,
    TRAIL_CHANGED,
};
use crate::state::{AppState, LockExt};

/// Feed one coordinate sample through the tracker and notify the UI only when
/// the sample became confirmed state.
pub fn ingest_sample(app: &AppHandle, x: f64, y: f64, z: f64) {
    ingest_sample_with_heading(app, x, y, z, None);
}

pub fn ingest_sample_with_heading(
    app: &AppHandle,
    x: f64,
    y: f64,
    z: f64,
    heading_deg: Option<f64>,
) {
    let state = app.state::<AppState>();
    let now_s = state.now_s();
    let cal = state.active_calibration();

    let (outcome, current, heading, velocity, trail) = {
        let mut tracker = state.tracker.lock_safe();
        let outcome = tracker.add_sample_with_heading(x, y, z, heading_deg, now_s);
        let current = tracker.current;
        let heading = tracker.heading_with_source(now_s);
        let velocity = tracker.velocity_cm_s();
        let trail = outcome
            .trail_changed
            .then(|| trail_payload(&tracker.segments, cal));
        (outcome, current, heading, velocity, trail)
    };

    if !should_publish(outcome) {
        log::warn!("position sample quarantined as implausible");
        return;
    }

    if should_persist(outcome) {
        if let Some(writer) = state.trail_writer.lock_safe().as_mut() {
            if outcome.broke_segment {
                writer.add_break();
            }
            writer.add(x, y, z);
        }
    }

    let payload = position_payload(
        current.expect("accepted sample is current"),
        heading,
        velocity,
        now_s,
        cal,
    );
    emit_all(app, POSITION_UPDATE, payload);
    if let Some(trail) = trail {
        emit_all(app, TRAIL_CHANGED, trail);
    }
}

fn should_publish(outcome: SampleOutcome) -> bool {
    outcome.accepted
}

fn should_persist(outcome: SampleOutcome) -> bool {
    outcome.accepted && !outcome.refreshed_only
}

const PREDICTION_HORIZON_S: f64 = 4.0;
const STALE_AFTER_S: f64 = 12.0;

fn heading_source_key(source: HeadingSource) -> &'static str {
    match source {
        HeadingSource::Server => "server",
        HeadingSource::Motion => "motion",
    }
}

fn position_payload(
    current: Sample,
    heading: Option<(f64, HeadingSource)>,
    velocity: Option<(f64, f64)>,
    now_s: f64,
    cal: &Calibration,
) -> PositionUpdate {
    let (px, py) = world_to_pixel(current.x, current.y, cal);
    let ((velocity_x_cm_s, velocity_y_cm_s), (velocity_px_x_s, velocity_px_y_s)) =
        match velocity {
            Some((vx, vy)) => {
                let (next_px, next_py) = world_to_pixel(current.x + vx, current.y + vy, cal);
                (
                    (Some(vx), Some(vy)),
                    (Some(next_px - px), Some(next_py - py)),
                )
            }
            None => ((None, None), (None, None)),
        };
    let age_ms = ((now_s - current.at_s).max(0.0) * 1000.0).round() as i64;
    let heading_deg = heading.map(|(degrees, _)| degrees);
    PositionUpdate {
        x_cm: current.x,
        y_cm: current.y,
        z_cm: current.z,
        px,
        py,
        heading_deg,
        heading_source: heading.map(|(_, source)| heading_source_key(source)),
        compass_key: heading_deg.map(bearing_to_compass_key),
        velocity_x_cm_s,
        velocity_y_cm_s,
        velocity_px_x_s,
        velocity_px_y_s,
        confirmed_at_ms: chrono::Utc::now().timestamp_millis() - age_ms,
        prediction_horizon_s: PREDICTION_HORIZON_S,
        stale_after_s: STALE_AFTER_S,
        in_bounds: overlay_core::is_in_bounds(px, py, cal),
    }
}

/// The current tracker state as a PositionUpdate, or None before the first
/// sample. Shared by `resync` and the `get_current_position` command so a
/// freshly (re)loaded webview paints at once instead of waiting for the
/// player's next manual coordinate copy.
pub fn current_payload(state: &AppState) -> Option<PositionUpdate> {
    let now_s = state.now_s();
    let cal = state.active_calibration();
    let (current, heading, velocity) = {
        let tracker = state.tracker.lock_safe();
        (
            tracker.current,
            tracker.heading_with_source(now_s),
            tracker.velocity_cm_s(),
        )
    };
    let cur = current?;
    Some(position_payload(cur, heading, velocity, now_s, cal))
}

/// Re-send the full current state to every window. Belt-and-braces: hidden
/// windows receive broadcasts and reloads fetch get_current_position, so
/// this mostly matters after a manual webview reload.
pub fn resync(app: &AppHandle) {
    let state = app.state::<AppState>();
    let cal = state.active_calibration();

    let trail = {
        let tracker = state.tracker.lock_safe();
        trail_payload(&tracker.segments, cal)
    };
    if let Some(payload) = current_payload(&state) {
        emit_all(app, POSITION_UPDATE, payload);
    }
    emit_all(app, TRAIL_CHANGED, trail);
    {
        let settings = state.settings.lock_safe().clone();
        emit_all(app, SETTINGS_CHANGED, settings);
    }
    emit_all(app, "waypoints://changed", ());
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

#[cfg(test)]
mod tests {
    use overlay_core::SampleOutcome;

    #[test]
    fn quarantined_sample_is_not_published_or_persisted() {
        let rejected = SampleOutcome {
            rejected_outlier: true,
            ..SampleOutcome::default()
        };
        assert!(!super::should_publish(rejected));
        assert!(!super::should_persist(rejected));
    }

    #[test]
    fn accepted_duplicate_updates_heading_but_not_the_trail_file() {
        let refreshed = SampleOutcome {
            accepted: true,
            refreshed_only: true,
            ..SampleOutcome::default()
        };
        assert!(super::should_publish(refreshed));
        assert!(!super::should_persist(refreshed));
    }
}
