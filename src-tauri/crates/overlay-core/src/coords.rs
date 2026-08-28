//! World <-> map-pixel transform and geometry. 1:1 port of the transform half
//! of `app/coords.py`.
//!
//! Transform taken from VulnonaMAP's real source ($map.calc.game2map in
//! js/map.js) and cross-checked against myislemap.com/app.js gameToMap() —
//! two different authors, same constants.
//!
//! ```text
//! "Asset Location" field 1 = Lat  = game X -> VERTICAL image axis
//! "Asset Location" field 2 = Long = game Y -> HORIZONTAL image axis
//! ```

use crate::calibration::Calibration;

/// Game coords (cm) -> pixel on the original map image.
///
/// Never clamps the result: some real locations sit outside the charted image
/// (Hell's Mouth is ~995 m past the southern edge). The caller decides whether
/// to draw an edge arrow or skip.
pub fn world_to_pixel(x_cm: f64, y_cm: f64, cal: &Calibration) -> (f64, f64) {
    let px = (y_cm / 1000.0 - cal.min_y) / cal.span_y() * cal.image_width_px as f64;
    let py = (x_cm / 1000.0 - cal.min_x) / cal.span_x() * cal.image_height_px as f64;
    (px, py)
}

/// Pixel on the map image -> game coords (cm). Used for click-to-place-waypoint.
pub fn pixel_to_world(px: f64, py: f64, cal: &Calibration) -> (f64, f64) {
    let x_cm = (py / cal.image_height_px as f64 * cal.span_x() + cal.min_x) * 1000.0;
    let y_cm = (px / cal.image_width_px as f64 * cal.span_y() + cal.min_y) * 1000.0;
    (x_cm, y_cm)
}

pub fn is_in_bounds(px: f64, py: f64, cal: &Calibration) -> bool {
    (0.0..=cal.image_width_px as f64).contains(&px)
        && (0.0..=cal.image_height_px as f64).contains(&py)
}

/// Real-world distance in metres. Computed straight from cm, never via pixels.
pub fn distance_m(x1_cm: f64, y1_cm: f64, x2_cm: f64, y2_cm: f64) -> f64 {
    (x2_cm - x1_cm).hypot(y2_cm - y1_cm) / 100.0
}

/// Compass bearing 0-360 from point 1 to point 2 (0 = North, 90 = East).
///
/// AXIS ORIENTATION (verified against named in-game landmarks — do not
/// re-derive):
///
/// ```text
/// gameX INCREASES -> going SOUTH (down on the image)
/// gameY INCREASES -> going EAST  (right on the image)
/// ```
///
/// Evidence: "Highland Lake - north" and "- south" are two halves of the same
/// lake and the "north" half has the SMALLER gameX; "Swamp East" has a larger
/// gameY than "Swamp West". The northward component is therefore -dX, not +dX
/// — this was once wrong and silently reversed the N/S compass labels while
/// E/W stayed correct.
pub fn bearing_deg(x1_cm: f64, y1_cm: f64, x2_cm: f64, y2_cm: f64, cal: &Calibration) -> f64 {
    let north = -(x2_cm - x1_cm);
    let east = y2_cm - y1_cm;
    (east.atan2(north).to_degrees() + cal.north_offset_deg).rem_euclid(360.0)
}

const COMPASS_KEYS: [&str; 8] = [
    "dir.N", "dir.NE", "dir.E", "dir.SE", "dir.S", "dir.SW", "dir.W", "dir.NW",
];

/// Bearing -> direction string KEY (a key, not display text, so it can be
/// localised).
pub fn bearing_to_compass_key(bearing: f64) -> &'static str {
    COMPASS_KEYS[((bearing.rem_euclid(360.0) / 45.0 + 0.5) as usize) % 8]
}

/// Convert The Isle / Unreal's actor yaw to this overlay's north-up compass.
/// Unreal yaw 0 faces +gameX (south on the Gateway map), while positive yaw
/// turns toward +gameY (east), hence the mirrored `180 - yaw` transform.
pub fn game_yaw_to_bearing(yaw: f64) -> f64 {
    (180.0 - yaw).rem_euclid(360.0)
}
