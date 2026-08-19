//! Tests for the tracker logic. The original Python app had none — these pin
//! the behaviors documented in tracker.py's comments before the port gets
//! built on.

use overlay_core::calibration::Calibration;
use overlay_core::tracker::{PositionTracker, TrailConfig, HEADING_MAX_AGE_S};

fn tracker() -> PositionTracker {
    PositionTracker::new(Calibration::gateway().clone(), TrailConfig::default())
}

#[test]
fn first_sample_starts_a_segment_with_one_node() {
    let mut t = tracker();
    let out = t.add_sample(1000.0, 2000.0, 0.0, 0.0);
    assert!(out.trail_changed);
    assert!(out.broke_segment);
    assert!(!out.refreshed_only);
    assert_eq!(t.segments, vec![vec![(1000.0, 2000.0)]]);
    assert!(t.current.is_some());
    assert!(t.previous.is_none());
}

#[test]
fn same_spot_only_refreshes_timestamp() {
    let mut t = tracker();
    t.add_sample(1000.0, 2000.0, 0.0, 0.0);
    // < 1 cm away: timestamp refresh only, no node, nothing written.
    let out = t.add_sample(1000.0, 2000.5, 0.0, 100.0);
    assert!(out.refreshed_only);
    assert!(!out.trail_changed);
    assert_eq!(t.segments, vec![vec![(1000.0, 2000.0)]]);
    assert_eq!(t.current.unwrap().at_s, 100.0, "timestamp must refresh");
    assert!(t.previous.is_none(), "refresh must not rotate previous");
}

#[test]
fn small_move_updates_position_without_new_node() {
    let mut t = tracker();
    t.add_sample(0.0, 0.0, 0.0, 0.0);
    // 3 m: above refresh epsilon, below min_node_m (5 m).
    let out = t.add_sample(300.0, 0.0, 0.0, 10.0);
    assert!(!out.refreshed_only);
    assert!(!out.trail_changed);
    assert_eq!(t.segments, vec![vec![(0.0, 0.0)]]);
    assert!(t.previous.is_some());
}

#[test]
fn normal_move_appends_node_to_current_segment() {
    let mut t = tracker();
    t.add_sample(0.0, 0.0, 0.0, 0.0);
    let out = t.add_sample(10_000.0, 0.0, 0.0, 10.0); // 100 m
    assert!(out.trail_changed);
    assert!(!out.broke_segment);
    assert_eq!(t.segments, vec![vec![(0.0, 0.0), (10_000.0, 0.0)]]);
}

#[test]
fn long_jump_breaks_segment_and_starts_new_one_at_the_point() {
    let mut t = tracker();
    t.add_sample(0.0, 0.0, 0.0, 0.0);
    // 300 m > break_after_m (200 m): new segment starts AT this point so the
    // first point of the new leg is not lost.
    let out = t.add_sample(30_000.0, 0.0, 0.0, 10.0);
    assert!(out.broke_segment);
    assert!(out.trail_changed);
    assert_eq!(
        t.segments,
        vec![vec![(0.0, 0.0)], vec![(30_000.0, 0.0)]]
    );
}

#[test]
fn long_time_gap_breaks_segment() {
    let mut t = tracker();
    t.add_sample(0.0, 0.0, 0.0, 0.0);
    // 100 m moved but 16 minutes elapsed (> break_after_minutes = 15).
    let out = t.add_sample(10_000.0, 0.0, 0.0, 16.0 * 60.0);
    assert!(out.broke_segment);
    assert_eq!(
        t.segments,
        vec![vec![(0.0, 0.0)], vec![(10_000.0, 0.0)]]
    );
}

#[test]
fn heading_needs_two_samples_and_enough_distance() {
    let mut t = tracker();
    assert_eq!(t.heading(0.0), None);
    t.add_sample(0.0, 0.0, 0.0, 0.0);
    assert_eq!(t.heading(0.0), None, "one sample is not a direction");
    // 10 m: below HEADING_MIN_DISTANCE_M (20 m) -> still unsure.
    t.add_sample(1000.0, 0.0, 0.0, 10.0);
    assert_eq!(t.heading(10.0), None);
    // 100 m south (gameX increases): trustworthy, and south = 180.
    t.add_sample(11_000.0, 0.0, 0.0, 20.0);
    let h = t.heading(20.0).expect("heading must be available");
    assert!((h - 180.0).abs() < 1e-6, "got {h}");
}

#[test]
fn heading_expires_after_max_age() {
    let mut t = tracker();
    t.add_sample(0.0, 0.0, 0.0, 0.0);
    t.add_sample(10_000.0, 0.0, 0.0, 10.0);
    assert!(t.heading(10.0).is_some());
    assert_eq!(
        t.heading(10.0 + HEADING_MAX_AGE_S + 1.0),
        None,
        "a stale sample must not keep an arrow pointing"
    );
}

#[test]
fn bearing_to_reports_bearing_and_metres() {
    let mut t = tracker();
    assert_eq!(t.bearing_to(0.0, 0.0), None);
    t.add_sample(0.0, 0.0, 0.0, 0.0);
    let (bearing, dist) = t.bearing_to(0.0, 50_000.0).unwrap();
    assert!((bearing - 90.0).abs() < 1e-6, "east, got {bearing}");
    assert!((dist - 500.0).abs() < 1e-6, "500 m, got {dist}");
}

#[test]
fn clear_trail_resets_segments() {
    let mut t = tracker();
    t.add_sample(0.0, 0.0, 0.0, 0.0);
    t.add_sample(10_000.0, 0.0, 0.0, 10.0);
    t.clear_trail();
    assert_eq!(t.segments, vec![Vec::<(f64, f64)>::new()]);
}
