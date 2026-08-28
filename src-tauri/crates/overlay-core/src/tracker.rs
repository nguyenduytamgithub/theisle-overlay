//! Player position tracking over discrete samples. Port of `app/tracker.py`
//! with the Qt signals and disk writer factored out: `add_sample` returns a
//! [`SampleOutcome`] describing what happened, and the caller (the tauri app)
//! writes the trail JSONL and emits events from that.
//!
//! Samples are IRREGULAR by nature: the player copies coordinates whenever
//! they feel like it — two consecutive samples can be 5 seconds or 2 hours
//! apart. Everything here is designed around that fact:
//!
//!   - Heading is derived from the vector between the last two samples, and
//!     only trusted when they are close enough in time and far enough apart in
//!     distance. An arrow pointing the wrong way is worse than no arrow.
//!   - The trail breaks into segments when the gap in distance or time is too
//!     large, instead of drawing a straight line across it.

use std::collections::VecDeque;

use crate::calibration::Calibration;
use crate::coords::{bearing_deg, distance_m};

/// Confidence thresholds for the heading arrow.
pub const HEADING_MIN_DISTANCE_M: f64 = 20.0;
pub const HEADING_MAX_AGE_S: f64 = 600.0; // 10 minutes

/// The fastest accepted movement still leaves room above the fastest normal
/// dinosaur, while rejecting the field-observed 60+ m/s and kilometre spikes.
pub const MAX_PLAUSIBLE_SPEED_MPS: f64 = 25.0;
pub const POSITION_UNCERTAINTY_M: f64 = 25.0;
const HEADING_HISTORY_LEN: usize = 8;

/// Re-copying the same spot only refreshes the timestamp below this distance.
pub const REFRESH_EPSILON_M: f64 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Seconds on whatever monotonic-enough clock the caller uses; only
    /// differences matter.
    pub at_s: f64,
    /// Compass bearing already adapted at the source boundary.
    pub heading_deg: Option<f64>,
}

impl Sample {
    pub fn age_s(&self, now_s: f64) -> f64 {
        now_s - self.at_s
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrailConfig {
    pub enabled: bool,
    pub break_after_s: f64,
    pub break_after_m: f64,
    pub min_node_m: f64,
}

impl Default for TrailConfig {
    fn default() -> Self {
        // Mirrors DEFAULT_SETTINGS["trail"] in the original config.py.
        Self {
            enabled: true,
            break_after_s: 15.0 * 60.0,
            break_after_m: 200.0,
            min_node_m: 5.0,
        }
    }
}

/// What one `add_sample` call did — drives the caller's JSONL writes and
/// change events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SampleOutcome {
    /// The sample became the confirmed current position (duplicates included).
    pub accepted: bool,
    /// Same spot re-copied: only the timestamp was refreshed; no node was
    /// added and the sample must NOT be written to the trail file.
    pub refreshed_only: bool,
    /// A segment break happened — the caller writes a `break` record before
    /// the sample.
    pub broke_segment: bool,
    /// The visible trail changed (node added and/or segment broken).
    pub trail_changed: bool,
    /// The sample was quarantined as physically implausible.
    pub rejected_outlier: bool,
    /// A second consistent outlier confirmed a real relocation/respawn.
    pub relocated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingSource {
    Server,
    Motion,
}

/// Holds the current position, the previous one, and the session trail.
#[derive(Debug, Clone)]
pub struct PositionTracker {
    cal: Calibration,
    config: TrailConfig,
    pub current: Option<Sample>,
    pub previous: Option<Sample>,
    /// Trail polylines in world cm; a new inner Vec starts after each break.
    pub segments: Vec<Vec<(f64, f64)>>,
    pending_jump: Option<Sample>,
    history: VecDeque<Sample>,
    velocity_cm_s: Option<(f64, f64)>,
}

impl PositionTracker {
    pub fn new(cal: Calibration, config: TrailConfig) -> Self {
        Self {
            cal,
            config,
            current: None,
            previous: None,
            segments: Vec::new(),
            pending_jump: None,
            history: VecDeque::with_capacity(HEADING_HISTORY_LEN),
            velocity_cm_s: None,
        }
    }

    // -- receiving new samples --------------------------------------------

    pub fn add_sample(&mut self, x: f64, y: f64, z: f64, now_s: f64) -> SampleOutcome {
        self.add_sample_with_heading(x, y, z, None, now_s)
    }

    pub fn add_sample_with_heading(
        &mut self,
        x: f64,
        y: f64,
        z: f64,
        heading_deg: Option<f64>,
        now_s: f64,
    ) -> SampleOutcome {
        let heading_deg = heading_deg.filter(|v| v.is_finite()).map(|v| v.rem_euclid(360.0));
        let sample = Sample {
            x,
            y,
            z,
            at_s: now_s,
            heading_deg,
        };
        let mut outcome = SampleOutcome::default();

        if !x.is_finite() || !y.is_finite() || !z.is_finite() || !now_s.is_finite() {
            outcome.rejected_outlier = true;
            return outcome;
        }

        if let Some(current) = self.current {
            let moved = distance_m(current.x, current.y, x, y);
            let gap_s = now_s - current.at_s;
            if gap_s <= 0.0 {
                outcome.rejected_outlier = true;
                return outcome;
            }
            if moved < REFRESH_EPSILON_M {
                // Re-copied the same spot: refresh the timestamp only.
                self.current = Some(sample);
                self.velocity_cm_s = Some((0.0, 0.0));
                self.pending_jump = None;
                outcome.accepted = true;
                outcome.refreshed_only = true;
                return outcome;
            }

            if let Some(pending) = self.pending_jump {
                let pending_gap_s = now_s - pending.at_s;
                let pending_moved = distance_m(pending.x, pending.y, x, y);
                let pending_plausible = pending_gap_s > 0.0
                    && pending_moved
                        <= POSITION_UNCERTAINTY_M + MAX_PLAUSIBLE_SPEED_MPS * pending_gap_s;
                if pending_plausible {
                    self.start_new_segment();
                    self.append_node(x, y);
                    self.previous = None;
                    self.current = Some(sample);
                    self.pending_jump = None;
                    self.velocity_cm_s = None;
                    self.history.clear();
                    self.push_history(sample);
                    outcome.accepted = true;
                    outcome.relocated = true;
                    outcome.broke_segment = true;
                    outcome.trail_changed = true;
                    return outcome;
                }
            }

            let plausible_m = POSITION_UNCERTAINTY_M + MAX_PLAUSIBLE_SPEED_MPS * gap_s;
            if moved > plausible_m {
                self.pending_jump = Some(sample);
                outcome.rejected_outlier = true;
                return outcome;
            }

            self.pending_jump = None;
            self.velocity_cm_s = Some(((x - current.x) / gap_s, (y - current.y) / gap_s));
            if gap_s > self.config.break_after_s {
                // Break the segment and start the new one AT this point — if
                // we waited for the next sample, the first point of the new
                // leg would be lost.
                self.start_new_segment();
                self.append_node(x, y);
                outcome.broke_segment = true;
                outcome.trail_changed = true;
            } else if moved >= self.config.min_node_m {
                self.append_node(x, y);
                outcome.trail_changed = true;
            }
        } else {
            self.start_new_segment();
            self.append_node(x, y);
            outcome.broke_segment = true;
            outcome.trail_changed = true;
        }

        self.previous = self.current;
        self.current = Some(sample);
        self.push_history(sample);
        outcome.accepted = true;
        outcome
    }

    fn push_history(&mut self, sample: Sample) {
        self.history.push_back(sample);
        while self.history.len() > HEADING_HISTORY_LEN {
            self.history.pop_front();
        }
    }

    fn start_new_segment(&mut self) {
        if matches!(self.segments.last(), Some(s) if s.is_empty()) {
            self.segments.pop();
        }
        self.segments.push(Vec::new());
    }

    fn append_node(&mut self, x: f64, y: f64) {
        if self.segments.is_empty() {
            self.segments.push(Vec::new());
        }
        self.segments.last_mut().unwrap().push((x, y));
    }

    pub fn clear_trail(&mut self) {
        self.segments = vec![Vec::new()];
    }

    pub fn config(&self) -> &TrailConfig {
        &self.config
    }

    // -- derived state -----------------------------------------------------

    /// Compass bearing of travel, or None while not confident enough.
    pub fn heading(&self, now_s: f64) -> Option<f64> {
        self.heading_with_source(now_s).map(|(heading, _)| heading)
    }

    pub fn heading_with_source(&self, now_s: f64) -> Option<(f64, HeadingSource)> {
        let current = self.current?;
        if current.age_s(now_s) > HEADING_MAX_AGE_S {
            return None;
        }
        if let Some(heading) = current.heading_deg {
            return Some((heading, HeadingSource::Server));
        }
        let anchor = self.history.iter().find(|sample| {
            current.at_s - sample.at_s <= HEADING_MAX_AGE_S
                && distance_m(sample.x, sample.y, current.x, current.y)
                    >= HEADING_MIN_DISTANCE_M
        })?;
        Some((
            bearing_deg(anchor.x, anchor.y, current.x, current.y, &self.cal),
            HeadingSource::Motion,
        ))
    }

    pub fn velocity_cm_s(&self) -> Option<(f64, f64)> {
        self.velocity_cm_s
    }

    /// (bearing, distance in metres) from the current position to a point.
    pub fn bearing_to(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let current = self.current?;
        Some((
            bearing_deg(current.x, current.y, x, y, &self.cal),
            distance_m(current.x, current.y, x, y),
        ))
    }
}
