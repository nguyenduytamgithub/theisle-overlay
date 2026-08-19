//! Port of the original `tests/test_coords.py` — the arithmetic contract of
//! the whole app. No Qt, no game, no I/O: fixtures come from the embedded
//! calibration JSON so code and expectations cannot drift apart.

use overlay_core::calibration::{Calibration, SelfTest};
use overlay_core::coords::{
    bearing_deg, bearing_to_compass_key, distance_m, is_in_bounds, pixel_to_world,
    world_to_pixel,
};
use overlay_core::parse::{parse_coordinates, NumberFormat};

fn cal() -> &'static Calibration {
    Calibration::gateway()
}

fn assert_close(got: f64, want: f64, tol: f64, ctx: &str) {
    assert!(
        (got - want).abs() <= tol,
        "{ctx}: got {got}, want {want} (tol {tol})"
    );
}

// ---------------------------------------------------------------------------
// Golden anchors: 5 landmarks at +-1.0 px.
//
// Pure arithmetic — the expected values were derived independently from the
// VulnonaMAP source. The tolerance is kept tight to let real bugs through.
// ---------------------------------------------------------------------------

#[test]
fn anchors_land_on_expected_pixels() {
    let st = SelfTest::embedded();
    for a in &st.anchors {
        let (px, py) = world_to_pixel(a.raw[0], a.raw[1], cal());
        assert_close(px, a.px, st.tolerance_px, &format!("{} px", a.name));
        assert_close(py, a.py, st.tolerance_px, &format!("{} py", a.name));
    }
}

#[test]
fn anchors_are_in_bounds() {
    for a in &SelfTest::embedded().anchors {
        let (px, py) = world_to_pixel(a.raw[0], a.raw[1], cal());
        assert!(is_in_bounds(px, py, cal()), "{} must be in bounds", a.name);
    }
}

// ---------------------------------------------------------------------------
// NEGATIVE test: Hell's Mouth genuinely sits outside the map and must NOT be
// clamped. This assertion is what catches an axis swap — with X/Y exchanged
// the point lands inside the valid area and the test fails.
// ---------------------------------------------------------------------------

#[test]
fn hells_mouth_falls_below_image() {
    let st = SelfTest::embedded();
    let ob = &st.out_of_bounds;
    let (px, py) = world_to_pixel(ob.raw[0], ob.raw[1], cal());
    assert_close(px, ob.px, 1.0, "hells mouth px");
    assert_close(py, ob.py, 1.0, "hells mouth py");
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

/// game Y drives the HORIZONTAL axis, game X drives the VERTICAL axis.
#[test]
fn axis_assignment_is_not_swapped() {
    let (px0, py0) = world_to_pixel(0.0, 0.0, cal());
    let (px_y, py_y) = world_to_pixel(0.0, 100_000.0, cal()); // only Y changed
    let (px_x, py_x) = world_to_pixel(100_000.0, 0.0, cal()); // only X changed
    assert!(px_y > px0);
    assert_close(py_y, py0, 1e-9, "y-only move must not change py");
    assert!(py_x > py0);
    assert_close(px_x, px0, 1e-9, "x-only move must not change px");
}

// ---------------------------------------------------------------------------
// Round trips.
// ---------------------------------------------------------------------------

#[test]
fn anchors_round_trip() {
    for a in &SelfTest::embedded().anchors {
        let (px, py) = world_to_pixel(a.raw[0], a.raw[1], cal());
        let (x, y) = pixel_to_world(px, py, cal());
        assert_close(x, a.raw[0], 1.0, &format!("{} x round trip (1 cm)", a.name));
        assert_close(y, a.raw[1], 1.0, &format!("{} y round trip (1 cm)", a.name));
    }
}

/// Deterministic LCG so the fuzz run is reproducible without a rand
/// dependency (the original used random.Random(20260819); the exact stream
/// differs, which is fine — the property under test is the same).
struct Lcg(u64);

impl Lcg {
    fn next_unit(&mut self) -> f64 {
        // Numerical Recipes LCG constants.
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_unit()
    }
}

#[test]
fn fuzz_round_trip_10k_points() {
    let mut rng = Lcg(20260819);
    let mut worst: f64 = 0.0;
    for _ in 0..10_000 {
        let x = rng.uniform(cal().min_x * 1000.0, cal().max_x * 1000.0);
        let y = rng.uniform(cal().min_y * 1000.0, cal().max_y * 1000.0);
        let (px, py) = world_to_pixel(x, y, cal());
        assert!(is_in_bounds(px, py, cal()));
        let (rx, ry) = pixel_to_world(px, py, cal());
        worst = worst.max((rx - x).abs()).max((ry - y).abs());
    }
    assert!(worst < 1.0, "worst round-trip error {worst} cm");
}

// ---------------------------------------------------------------------------
// Parse: accept table.
// ---------------------------------------------------------------------------

fn parse(text: &str) -> Option<(f64, f64, f64)> {
    parse_coordinates(text, NumberFormat::Auto)
}

#[test]
fn parse_evrima_format() {
    let (x, y, z) = parse("-231,654.353, 52,099.673, 29,328.085").expect("must parse");
    assert_close(x, -231654.353, 1e-3, "x");
    assert_close(y, 52099.673, 1e-3, "y");
    assert_close(z, 29328.085, 1e-3, "z");
}

#[test]
fn parse_unicode_minus() {
    let (x, ..) = parse("\u{2212}231,654.353, 52,099.673, 29,328.085").expect("must parse");
    assert_close(x, -231654.353, 1e-3, "x");
}

#[test]
fn parse_legacy_lat_long_alt() {
    let (x, y, z) =
        parse("(Lat: -159,923.37 Long: 293,325.38 Alt: 20,976.557)").expect("must parse");
    assert_close(x, -159923.37, 1e-2, "x");
    assert_close(y, 293325.38, 1e-2, "y");
    assert_close(z, 20976.557, 1e-3, "z");
}

#[test]
fn parse_two_values_z_defaults_zero() {
    let (.., z) = parse("404,868.089, 436,554.633").expect("must parse");
    assert_eq!(z, 0.0);
}

#[test]
fn parse_eu_decimal_format() {
    let (x, y, _) = parse("-231.654,353, 52.099,673, 29.328,085").expect("must parse");
    assert_close(x, -231654.353, 1e-3, "x");
    assert_close(y, 52099.673, 1e-3, "y");
}

#[test]
fn parse_surrounding_whitespace_and_newlines() {
    let (x, ..) = parse("\n  -231,654.353, 52,099.673, 29,328.085  \r\n").expect("must parse");
    assert_close(x, -231654.353, 1e-3, "x");
}

/// US-style "{:,.3f}" formatting, to feed anchors through the parser exactly
/// the way the game prints them.
fn fmt_us(v: f64) -> String {
    let s = format!("{:.3}", v.abs());
    let (int_part, frac) = s.split_once('.').unwrap();
    let mut grouped = String::new();
    for (i, c) in int_part.chars().enumerate() {
        if i > 0 && (int_part.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{}{grouped}.{frac}", if v < 0.0 { "-" } else { "" })
}

/// Both halves joined: a string as the game emits it -> the right pixel.
#[test]
fn all_five_anchors_parse_and_map() {
    let st = SelfTest::embedded();
    for a in &st.anchors {
        let text = format!("{}, {}, 20,000.0", fmt_us(a.raw[0]), fmt_us(a.raw[1]));
        let (x, y, _) = parse(&text).unwrap_or_else(|| panic!("{text:?} must parse"));
        let (px, py) = world_to_pixel(x, y, cal());
        assert_close(px, a.px, st.tolerance_px, &format!("{} via {text:?}", a.name));
        assert_close(py, a.py, st.tolerance_px, &format!("{} via {text:?}", a.name));
    }
}

// ---------------------------------------------------------------------------
// Parse: reject table — as important as the accept table. The user's normal
// copy/paste must feel like nothing happened.
// ---------------------------------------------------------------------------

#[test]
fn rejects_non_coordinates() {
    let rubbish: &[String] = &[
        "".into(),
        "   ".into(),
        "hello".into(),
        "1,234".into(),
        "2026-08-19".into(),
        "0912 345 678".into(),
        "+84 912 345 678".into(),
        "a3f5c9e1b2d4a6f8c0e2b4d6a8f0c2e4b6d8a0f2".into(), // git SHA
        "C:\\Users\\hongd\\Desktop\\file123.txt".into(),
        "https://example.com/page/12345".into(),
        "version 1.2.3".into(),
        "0, 0, 0".into(),
        "x".repeat(5000),
        "The quick brown fox jumped over 3 lazy dogs".into(),
    ];
    for text in rubbish {
        assert!(
            parse(text).is_none(),
            "must reject {:?}",
            &text[..text.len().min(40)]
        );
    }
}

#[test]
fn rejects_absurd_magnitudes() {
    assert!(parse("99,999,999.0, 88,888,888.0, 1.0").is_none());
}

// ---------------------------------------------------------------------------
// Geometry.
// ---------------------------------------------------------------------------

#[test]
fn distance_is_metres() {
    assert_close(distance_m(0.0, 0.0, 30_000.0, 40_000.0), 500.0, 1e-6, "3-4-5");
}

#[test]
fn bearing_cardinals() {
    // gameX INCREASES = going SOUTH ; gameY INCREASES = going EAST
    assert_close(bearing_deg(0.0, 0.0, -1000.0, 0.0, cal()), 0.0, 1e-6, "north");
    assert_close(bearing_deg(0.0, 0.0, 0.0, 1000.0, cal()), 90.0, 1e-6, "east");
    assert_close(bearing_deg(0.0, 0.0, 1000.0, 0.0, cal()), 180.0, 1e-6, "south");
    assert_close(bearing_deg(0.0, 0.0, 0.0, -1000.0, cal()), 270.0, 1e-6, "west");
}

/// The compass must agree with movement on the map image.
///
/// The map draws north up, so: going north must DECREASE py, going east must
/// INCREASE px. This test ties the two systems together — if someone edits
/// the transform but forgets the compass (or vice versa) it fails.
#[test]
fn bearing_matches_map_screen_directions() {
    let cases: &[(f64, &str, (f64, f64), fn((f64, f64), (f64, f64)) -> bool)] = &[
        (0.0, "north", (-1000.0, 0.0), |p0, p1| p1.1 < p0.1), // north -> py down
        (90.0, "east", (0.0, 1000.0), |p0, p1| p1.0 > p0.0),  // east  -> px up
        (180.0, "south", (1000.0, 0.0), |p0, p1| p1.1 > p0.1), // south -> py up
        (270.0, "west", (0.0, -1000.0), |p0, p1| p1.0 < p0.0), // west  -> px down
    ];
    for (expected, name, (dx, dy), screen_check) in cases {
        assert_close(
            bearing_deg(0.0, 0.0, *dx, *dy, cal()),
            *expected,
            1e-6,
            name,
        );
        let p0 = world_to_pixel(0.0, 0.0, cal());
        let p1 = world_to_pixel(*dx, *dy, cal());
        assert!(
            screen_check(p0, p1),
            "direction {name} does not match image movement"
        );
    }
}

/// The root evidence: in-game landmarks with direction names. This is how we
/// know increasing gameX goes SOUTH, not north.
#[test]
fn named_landmarks_confirm_axis_orientation() {
    let north_lake = (-135000.0, -22000.0); // "Highland Lake - north"
    let south_lake = (-92000.0, -65000.0); // "Highland Lake - south"
    let swamp_east = (254000.0, 106000.0);
    let swamp_west = (276000.0, -30000.0);

    assert!(
        world_to_pixel(north_lake.0, north_lake.1, cal()).1
            < world_to_pixel(south_lake.0, south_lake.1, cal()).1,
        "the 'north' half of the lake must sit higher on the image"
    );
    assert!(
        world_to_pixel(swamp_east.0, swamp_east.1, cal()).0
            > world_to_pixel(swamp_west.0, swamp_west.1, cal()).0,
        "'East' must sit right of 'West'"
    );
    // And the compass must agree with those same landmarks. The two lake
    // halves are offset by exactly 43 km on both axes, so the bearing must be
    // 45 degrees — north-east.
    let bearing = bearing_deg(south_lake.0, south_lake.1, north_lake.0, north_lake.1, cal());
    assert_close(bearing, 45.0, 1e-6, "south lake -> north lake");
    assert_eq!(bearing_to_compass_key(bearing), "dir.NE");
    // The northward component must be positive: south lake -> north lake goes north.
    assert!(bearing.to_radians().cos() > 0.0);
}

#[test]
fn compass_keys() {
    for (bearing, key) in [
        (0.0, "dir.N"),
        (45.0, "dir.NE"),
        (90.0, "dir.E"),
        (135.0, "dir.SE"),
        (180.0, "dir.S"),
        (225.0, "dir.SW"),
        (270.0, "dir.W"),
        (315.0, "dir.NW"),
        (359.0, "dir.N"),
    ] {
        assert_eq!(bearing_to_compass_key(bearing), key, "bearing {bearing}");
    }
}
