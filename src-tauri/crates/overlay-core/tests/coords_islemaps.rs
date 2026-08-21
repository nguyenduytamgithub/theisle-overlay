//! Geometry contract of the islemaps.com calibration frame — the mirror of
//! the geometry half of `coords.rs`, run against the second embedded
//! calibration. The anchors are the same world-cm landmarks re-projected into
//! the 2500px islemaps frame.

use overlay_core::calibration::{Calibration, MapSource, SelfTest};
use overlay_core::coords::{is_in_bounds, pixel_to_world, world_to_pixel};
use overlay_core::CALIBRATION_ISLEMAPS_JSON;

fn cal() -> &'static Calibration {
    MapSource::IslemapsLight.calibration()
}

fn selftest() -> SelfTest {
    SelfTest::from_json(CALIBRATION_ISLEMAPS_JSON)
}

fn assert_close(got: f64, want: f64, tol: f64, ctx: &str) {
    assert!(
        (got - want).abs() <= tol,
        "{ctx}: got {got}, want {want} (tol {tol})"
    );
}

// ---------------------------------------------------------------------------
// Golden anchors: the 5 landmarks from the Vulnona suite plus Hell's Mouth,
// which is IN bounds here — the sharpest regression test of the larger
// islemaps extent.
// ---------------------------------------------------------------------------

#[test]
fn anchors_land_on_expected_pixels() {
    let st = selftest();
    for a in &st.anchors {
        let (px, py) = world_to_pixel(a.raw[0], a.raw[1], cal());
        assert_close(px, a.px, st.tolerance_px, &format!("{} px", a.name));
        assert_close(py, a.py, st.tolerance_px, &format!("{} py", a.name));
    }
}

#[test]
fn anchors_are_in_bounds_including_hells_mouth() {
    let st = selftest();
    let mut saw_hells_mouth = false;
    for a in &st.anchors {
        let (px, py) = world_to_pixel(a.raw[0], a.raw[1], cal());
        assert!(is_in_bounds(px, py, cal()), "{} must be in bounds", a.name);
        saw_hells_mouth |= a.name.starts_with("Hell's Mouth");
    }
    assert!(
        saw_hells_mouth,
        "the anchor list must keep Hell's Mouth — it is the extent regression test"
    );
}

// ---------------------------------------------------------------------------
// NEGATIVE test: a synthetic point past max_X must fall below the image and
// must NOT be clamped.
// ---------------------------------------------------------------------------

#[test]
fn synthetic_south_point_falls_below_image() {
    let st = selftest();
    let ob = &st.out_of_bounds;
    let (px, py) = world_to_pixel(ob.raw[0], ob.raw[1], cal());
    assert_close(px, ob.px, 1.0, "synthetic south px");
    assert_close(py, ob.py, 1.0, "synthetic south py");
    assert!(py > cal().image_height_px as f64);
    assert!(!is_in_bounds(px, py, cal()));
}

// ---------------------------------------------------------------------------
// Corners: guards against someone mis-editing the calibration constants.
// ---------------------------------------------------------------------------

#[test]
fn corner_top_left() {
    let (px, py) = world_to_pixel(cal().min_x * 1000.0, cal().min_y * 1000.0, cal());
    assert_close(px, 0.0, 0.01, "top-left px");
    assert_close(py, 0.0, 0.01, "top-left py");
}

#[test]
fn corner_bottom_right() {
    let (px, py) = world_to_pixel(cal().max_x * 1000.0, cal().max_y * 1000.0, cal());
    assert_close(px, cal().image_width_px as f64, 0.01, "bottom-right px");
    assert_close(py, cal().image_height_px as f64, 0.01, "bottom-right py");
}

// ---------------------------------------------------------------------------
// Round trip.
// ---------------------------------------------------------------------------

#[test]
fn anchors_round_trip() {
    for a in &selftest().anchors {
        let (px, py) = world_to_pixel(a.raw[0], a.raw[1], cal());
        let (x, y) = pixel_to_world(px, py, cal());
        assert_close(x, a.raw[0], 1.0, &format!("{} x round trip (1 cm)", a.name));
        assert_close(y, a.raw[1], 1.0, &format!("{} y round trip (1 cm)", a.name));
    }
}

// ---------------------------------------------------------------------------
// MapSource contract.
// ---------------------------------------------------------------------------

#[test]
fn source_keys_round_trip() {
    for src in [
        MapSource::Vulnona,
        MapSource::IslemapsLight,
        MapSource::IslemapsDark,
    ] {
        assert_eq!(MapSource::try_from_key(src.key()), Some(src));
        assert_eq!(MapSource::from_key(src.key()), src);
    }
    assert_eq!(MapSource::try_from_key("spiro"), None);
    // Lenient path: junk falls back to the imagery guaranteed to exist.
    assert_eq!(MapSource::from_key("spiro"), MapSource::Vulnona);
    assert_eq!(MapSource::from_key(""), MapSource::Vulnona);
    assert_eq!(MapSource::default(), MapSource::Vulnona);
}

#[test]
fn light_and_dark_share_one_calibration() {
    assert!(std::ptr::eq(
        MapSource::IslemapsLight.calibration(),
        MapSource::IslemapsDark.calibration(),
    ));
    assert!(!std::ptr::eq(
        MapSource::Vulnona.calibration(),
        MapSource::IslemapsLight.calibration(),
    ));
}

/// Encodes the researched fact that makes this feature more than an image
/// swap: the islemaps frame is a strict superset of Vulnona's world extent
/// (it shows the SE archipelago that the 0.21.7 render crops off).
#[test]
fn islemaps_extent_is_superset_of_vulnona() {
    let vul = MapSource::Vulnona.calibration();
    let isl = cal();
    assert!(isl.min_x <= vul.min_x, "north edge");
    assert!(isl.max_x >= vul.max_x, "south edge");
    assert!(isl.min_y <= vul.min_y, "west edge");
    assert!(isl.max_y >= vul.max_y, "east edge");
}
