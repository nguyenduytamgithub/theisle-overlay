//! Tests for the tracker logic. The original Python app had none — these pin
//! the behaviors documented in tracker.py's comments before the port gets
//! built on.

use overlay_core::calibration::Calibration;
use overlay_core::coords::game_yaw_to_bearing;
use overlay_core::tracker::{
    HeadingSource, PositionTracker, TrailConfig, HEADING_MAX_AGE_S,
};

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
fn long_jump_waits_for_confirmation_before_starting_a_new_segment() {
    let mut t = tracker();
    t.add_sample(0.0, 0.0, 0.0, 0.0);
    // 300 m in 10 seconds is above the plausible movement envelope. It is
    // quarantined until a second nearby sample proves this was a relocation.
    let first = t.add_sample(30_000.0, 0.0, 0.0, 10.0);
    assert!(!first.accepted);
    assert_eq!(t.segments, vec![vec![(0.0, 0.0)]]);

    let out = t.add_sample(30_100.0, 100.0, 0.0, 20.0);
    assert!(out.accepted);
    assert!(out.relocated);
    assert!(out.broke_segment);
    assert!(out.trail_changed);
    assert_eq!(
        t.segments,
        vec![vec![(0.0, 0.0)], vec![(30_100.0, 100.0)]]
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

#[test]
fn impossible_spike_is_quarantined_and_return_to_route_is_accepted() {
    let mut t = tracker();
    t.add_sample(18_167.0, -252_835.0, 0.0, 0.0);
    let confirmed = t.current;
    let trail = t.segments.clone();

    // Real field regression: about 7.9 km in three seconds, followed by a
    // return to the original route. The one bad point must never become the
    // displayed position or a persisted trail node.
    let spike = t.add_sample_with_heading(752_257.0, -240.0, 0.0, None, 3.0);
    assert!(!spike.accepted);
    assert!(spike.rejected_outlier);
    assert_eq!(t.current, confirmed);
    assert_eq!(t.segments, trail);

    let resumed = t.add_sample_with_heading(18_900.0, -253_100.0, 0.0, None, 9.0);
    assert!(resumed.accepted);
    assert!(!resumed.relocated);
    assert!(!resumed.broke_segment);
}

#[test]
fn two_consistent_far_samples_confirm_a_relocation_without_a_connecting_line() {
    let mut t = tracker();
    t.add_sample(0.0, 0.0, 0.0, 0.0);

    let first = t.add_sample_with_heading(500_000.0, 500_000.0, 0.0, Some(90.0), 5.0);
    assert!(!first.accepted);

    let second = t.add_sample_with_heading(500_600.0, 500_200.0, 0.0, Some(92.0), 10.0);
    assert!(second.accepted);
    assert!(second.relocated);
    assert!(second.broke_segment);
    assert_eq!(t.segments, vec![vec![(0.0, 0.0)], vec![(500_600.0, 500_200.0)]]);
    assert_eq!(t.velocity_cm_s(), None, "a relocation cannot seed prediction velocity");
    assert_eq!(t.heading_with_source(10.0), Some((92.0, HeadingSource::Server)));
}

#[test]
fn delayed_but_plausible_movement_does_not_break_the_trail() {
    let mut t = tracker();
    t.add_sample(0.0, 0.0, 0.0, 0.0);

    // 324 m in 34 s is fast but physically plausible. The legacy fixed 200 m
    // rule incorrectly broke this kind of delayed server update.
    let out = t.add_sample_with_heading(32_400.0, 0.0, 0.0, None, 34.0);
    assert!(out.accepted);
    assert!(!out.broke_segment);
    assert_eq!(t.segments.len(), 1);
}

#[test]
fn fresh_server_heading_wins_and_motion_is_the_fallback() {
    let mut t = tracker();
    t.add_sample_with_heading(0.0, 0.0, 0.0, Some(359.0), 0.0);
    t.add_sample_with_heading(0.0, 10_000.0, 0.0, Some(1.0), 10.0);

    assert_eq!(t.heading_with_source(10.0), Some((1.0, HeadingSource::Server)));
    assert_eq!(t.heading_with_source(HEADING_MAX_AGE_S + 11.0), None);

    let mut fallback = tracker();
    fallback.add_sample(0.0, 0.0, 0.0, 0.0);
    fallback.add_sample(0.0, 10_000.0, 0.0, 10.0);
    assert_eq!(fallback.heading_with_source(10.0), Some((90.0, HeadingSource::Motion)));
}

#[test]
fn motion_course_is_independent_from_server_facing() {
    let mut t = tracker();
    t.add_sample_with_heading(0.0, 0.0, 0.0, Some(270.0), 0.0);
    t.add_sample_with_heading(0.0, 10_000.0, 0.0, Some(5.0), 10.0);
    assert_eq!(t.server_facing(10.0), Some(5.0));
    assert_eq!(t.motion_course(10.0), Some(90.0));
}

#[test]
fn unreal_yaw_converts_to_north_up_compass_bearing() {
    assert!((game_yaw_to_bearing(0.0) - 180.0).abs() < 1e-9);
    assert!((game_yaw_to_bearing(90.0) - 90.0).abs() < 1e-9);
    assert!((game_yaw_to_bearing(-90.0) - 270.0).abs() < 1e-9);
    assert!((game_yaw_to_bearing(540.0) - 0.0).abs() < 1e-9);
}
