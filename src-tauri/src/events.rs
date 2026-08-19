//! Event names and payload shapes shared with the frontend. Rust emits KEYS
//! and numbers only, never display strings — localisation happens in the
//! webview.

use serde::Serialize;

pub const POSITION_UPDATE: &str = "position://update";
pub const TRAIL_CHANGED: &str = "trail://changed";
pub const SETTINGS_CHANGED: &str = "settings://changed";

/// Every payload carries both raw cm and precomputed px: the frontend never
/// runs a coordinate transform of its own.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionUpdate {
    pub x_cm: f64,
    pub y_cm: f64,
    pub z_cm: f64,
    pub px: f64,
    pub py: f64,
    pub heading_deg: Option<f64>,
    /// Compass key ("dir.N".."dir.NW") for the heading, when known.
    pub compass_key: Option<&'static str>,
    pub in_bounds: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrailPayload {
    pub segments_cm: Vec<Vec<(f64, f64)>>,
    pub segments_px: Vec<Vec<(f64, f64)>>,
}
