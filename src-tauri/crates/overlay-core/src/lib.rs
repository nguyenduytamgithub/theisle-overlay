//! overlay-core — pure coordinate/tracking logic for The Isle overlay.
//!
//! This crate is deliberately free of I/O, OS calls, and tauri: it is the port
//! of the original app's `coords.py` + `tracker.py`, and it carries the whole
//! test suite (see `tests/`). Everything else depends on it being right.

pub mod calibration;
pub mod coords;
pub mod parse;
pub mod tracker;

pub use calibration::{Calibration, MapSource, CALIBRATION_ISLEMAPS_JSON, CALIBRATION_JSON};
pub use coords::{
    bearing_deg, bearing_to_compass_key, distance_m, is_in_bounds, pixel_to_world,
    world_to_pixel,
};
pub use parse::{parse_coordinates, NumberFormat};
pub use tracker::{HeadingSource, PositionTracker, Sample, SampleOutcome, TrailConfig};
