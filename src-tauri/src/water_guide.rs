//! Player-controlled guidance toward verified freshwater geometry.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use image::RgbaImage;
use overlay_core::{distance_m, pixel_to_world, Calibration};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, State};

use crate::events::PositionUpdate;
use crate::state::{AppState, LockExt};
use crate::{events, fetch, pipeline, settings};

const WATER_ALPHA_MIN: u8 = 128;
const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterGuideError {
    MissingFreshwater,
    InvalidFreshwater,
    UnsupportedMap,
    EmptyFreshwater,
    MissingWaterLabels,
    InvalidPois,
    WaitingForPosition,
}

impl WaterGuideError {
    pub fn key(self) -> &'static str {
        match self {
            Self::MissingFreshwater => "missing_freshwater",
            Self::InvalidFreshwater => "invalid_freshwater",
            Self::UnsupportedMap => "unsupported_map",
            Self::EmptyFreshwater => "empty_freshwater",
            Self::MissingWaterLabels => "missing_water_labels",
            Self::InvalidPois => "invalid_pois",
            Self::WaitingForPosition => "waiting_for_position",
        }
    }
}

impl fmt::Display for WaterGuideError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.key())
    }
}

impl std::error::Error for WaterGuideError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssetIdentity {
    len: u64,
    modified_ns: u128,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FreshwaterTarget {
    pub label: String,
    pub x_cm: f64,
    pub y_cm: f64,
    pub mask_px: [u32; 2],
    pub distance_m: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WaterGuideRoute {
    pub start_x_cm: f64,
    pub start_y_cm: f64,
    pub target_x_cm: f64,
    pub target_y_cm: f64,
    pub target_mask_px: [u32; 2],
    pub label: String,
    pub initial_distance_m: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WaterGuideSnapshot {
    pub requested: bool,
    pub route: Option<WaterGuideRoute>,
    pub error_key: Option<String>,
}

#[derive(Debug, Default)]
pub struct FreshwaterCache {
    mask_identity: Option<AssetIdentity>,
    pois_identity: Option<AssetIdentity>,
    candidates: Vec<(u32, u32)>,
    pois: Option<Value>,
    dimensions: Option<(u32, u32)>,
}

#[derive(Debug, Default)]
pub struct WaterGuideRuntime {
    requested: bool,
    route: Option<WaterGuideRoute>,
    error_key: Option<String>,
    cache: FreshwaterCache,
}

impl WaterGuideRuntime {
    pub fn snapshot(&self) -> WaterGuideSnapshot {
        WaterGuideSnapshot {
            requested: self.requested,
            route: self.route.clone(),
            error_key: self.error_key.clone(),
        }
    }

    fn toggle_with_position<F>(
        &mut self,
        position: Option<(f64, f64)>,
        selector: F,
    ) -> WaterGuideSnapshot
    where
        F: FnOnce(&mut FreshwaterCache, f64, f64) -> Result<FreshwaterTarget, WaterGuideError>,
    {
        if self.requested {
            self.requested = false;
            self.route = None;
            self.error_key = None;
            return self.snapshot();
        }

        self.requested = true;
        self.route = None;
        self.error_key = None;
        let Some((start_x_cm, start_y_cm)) = position else {
            self.error_key = Some(WaterGuideError::WaitingForPosition.key().into());
            return self.snapshot();
        };
        match selector(&mut self.cache, start_x_cm, start_y_cm) {
            Ok(target) => {
                self.route = Some(WaterGuideRoute {
                    start_x_cm,
                    start_y_cm,
                    target_x_cm: target.x_cm,
                    target_y_cm: target.y_cm,
                    target_mask_px: target.mask_px,
                    label: target.label,
                    initial_distance_m: target.distance_m,
                });
            }
            Err(error) => self.error_key = Some(error.key().into()),
        }
        self.snapshot()
    }
}

fn is_water(mask: &RgbaImage, x: i32, y: i32) -> bool {
    x >= 0
        && y >= 0
        && (x as u32) < mask.width()
        && (y as u32) < mask.height()
        && mask.get_pixel(x as u32, y as u32).0[3] >= WATER_ALPHA_MIN
}

fn water_neighbour_count(mask: &RgbaImage, x: u32, y: u32) -> u8 {
    NEIGHBOURS
        .iter()
        .filter(|&&(dx, dy)| is_water(mask, x as i32 + dx, y as i32 + dy))
        .count() as u8
}

fn inset_from_boundary(mask: &RgbaImage, x: u32, y: u32) -> (u32, u32) {
    let mut best = (x, y);
    let mut best_score = water_neighbour_count(mask, x, y);
    for (dx, dy) in NEIGHBOURS {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if !is_water(mask, nx, ny) {
            continue;
        }
        let candidate = (nx as u32, ny as u32);
        let score = water_neighbour_count(mask, candidate.0, candidate.1);
        if score > best_score || (score == best_score && candidate < best) {
            best = candidate;
            best_score = score;
        }
    }
    best
}

pub(crate) fn shallow_candidates(mask: &RgbaImage) -> Vec<(u32, u32)> {
    let mut candidates = BTreeSet::new();
    for y in 0..mask.height() {
        for x in 0..mask.width() {
            if !is_water(mask, x as i32, y as i32) {
                continue;
            }
            let boundary = NEIGHBOURS
                .iter()
                .any(|&(dx, dy)| !is_water(mask, x as i32 + dx, y as i32 + dy));
            if boundary {
                candidates.insert(inset_from_boundary(mask, x, y));
            }
        }
    }
    candidates.into_iter().collect()
}

pub(crate) fn nearest_candidate(
    candidates: &[(u32, u32)],
    player_x_cm: f64,
    player_y_cm: f64,
    calibration: &Calibration,
) -> Option<(u32, u32)> {
    candidates.iter().copied().min_by(|left, right| {
        let left_world = pixel_to_world(left.0 as f64 + 0.5, left.1 as f64 + 0.5, calibration);
        let right_world = pixel_to_world(right.0 as f64 + 0.5, right.1 as f64 + 0.5, calibration);
        let left_distance = distance_m(player_x_cm, player_y_cm, left_world.0, left_world.1);
        let right_distance = distance_m(player_x_cm, player_y_cm, right_world.0, right_world.1);
        left_distance
            .total_cmp(&right_distance)
            .then(left.cmp(right))
    })
}

fn validate_mask_dimensions(
    width: u32,
    height: u32,
    calibration: &Calibration,
) -> Result<(), WaterGuideError> {
    if width == calibration.image_width_px && height == calibration.image_height_px {
        Ok(())
    } else {
        Err(WaterGuideError::InvalidFreshwater)
    }
}

fn validate_poi_map(pois: &Value) -> Result<(), WaterGuideError> {
    if pois.get("map").and_then(Value::as_str) == Some(fetch::MAP_VERSION) {
        Ok(())
    } else {
        Err(WaterGuideError::UnsupportedMap)
    }
}

fn nearest_water_label(
    pois: &Value,
    target_x_cm: f64,
    target_y_cm: f64,
) -> Result<String, WaterGuideError> {
    let items = pois
        .pointer("/layers/water/items")
        .and_then(Value::as_array)
        .ok_or(WaterGuideError::MissingWaterLabels)?;
    items
        .iter()
        .filter_map(|item| {
            let label = item.get("label")?.as_str()?.trim();
            let x = item.get("x")?.as_f64()?;
            let y = item.get("y")?.as_f64()?;
            (!label.is_empty()).then(|| {
                (
                    distance_m(target_x_cm, target_y_cm, x, y),
                    label.to_string(),
                )
            })
        })
        .min_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
        })
        .map(|(_, label)| label)
        .ok_or(WaterGuideError::MissingWaterLabels)
}

fn asset_identity(path: &Path, missing: WaterGuideError) -> Result<AssetIdentity, WaterGuideError> {
    let bytes = fs::read(path).map_err(|_| missing)?;
    let metadata = fs::metadata(path).map_err(|_| missing)?;
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |value| value.as_nanos());
    let digest = Sha256::digest(&bytes);
    let sha256 = digest.iter().map(|byte| format!("{byte:02X}")).collect();
    Ok(AssetIdentity {
        len: metadata.len(),
        modified_ns,
        sha256,
    })
}

fn load_mask(path: &Path) -> Result<RgbaImage, WaterGuideError> {
    image::ImageReader::open(path)
        .map_err(|_| WaterGuideError::MissingFreshwater)?
        .decode()
        .map_err(|_| WaterGuideError::InvalidFreshwater)
        .map(|image| image.to_rgba8())
}

fn load_pois(path: &Path) -> Result<Value, WaterGuideError> {
    let bytes = fs::read(path).map_err(|_| WaterGuideError::InvalidPois)?;
    serde_json::from_slice(&bytes).map_err(|_| WaterGuideError::InvalidPois)
}

fn refresh_cache(cache: &mut FreshwaterCache) -> Result<(), WaterGuideError> {
    let mask_path = settings::islemaps_dir().join("freshwater.png");
    let pois_path = settings::pois_path();
    let mask_identity = asset_identity(&mask_path, WaterGuideError::MissingFreshwater)?;
    let pois_identity = asset_identity(&pois_path, WaterGuideError::InvalidPois)?;
    if cache.mask_identity.as_ref() == Some(&mask_identity)
        && cache.pois_identity.as_ref() == Some(&pois_identity)
    {
        return Ok(());
    }

    let mask = load_mask(&mask_path)?;
    validate_mask_dimensions(mask.width(), mask.height(), Calibration::islemaps())?;
    let candidates = shallow_candidates(&mask);
    if candidates.is_empty() {
        return Err(WaterGuideError::EmptyFreshwater);
    }
    let pois = load_pois(&pois_path)?;
    validate_poi_map(&pois)?;
    nearest_water_label(&pois, 0.0, 0.0)?;

    cache.mask_identity = Some(mask_identity);
    cache.pois_identity = Some(pois_identity);
    cache.candidates = candidates;
    cache.pois = Some(pois);
    cache.dimensions = Some((mask.width(), mask.height()));
    Ok(())
}

pub fn select_freshwater_target(
    cache: &mut FreshwaterCache,
    player_x_cm: f64,
    player_y_cm: f64,
) -> Result<FreshwaterTarget, WaterGuideError> {
    refresh_cache(cache)?;
    let calibration = Calibration::islemaps();
    let (px, py) = nearest_candidate(&cache.candidates, player_x_cm, player_y_cm, calibration)
        .ok_or(WaterGuideError::EmptyFreshwater)?;
    let (x_cm, y_cm) = pixel_to_world(px as f64 + 0.5, py as f64 + 0.5, calibration);
    let label = nearest_water_label(
        cache.pois.as_ref().ok_or(WaterGuideError::InvalidPois)?,
        x_cm,
        y_cm,
    )?;
    Ok(FreshwaterTarget {
        label,
        x_cm,
        y_cm,
        mask_px: [px, py],
        distance_m: distance_m(player_x_cm, player_y_cm, x_cm, y_cm),
    })
}

fn position_for_activation(
    position: Option<&PositionUpdate>,
    now_ms: i64,
) -> Result<(f64, f64), WaterGuideError> {
    let position = position.ok_or(WaterGuideError::WaitingForPosition)?;
    let age_ms = now_ms.saturating_sub(position.confirmed_at_ms);
    if !position.in_bounds
        || !position.x_cm.is_finite()
        || !position.y_cm.is_finite()
        || age_ms > 30_000
    {
        return Err(WaterGuideError::WaitingForPosition);
    }
    Ok((position.x_cm, position.y_cm))
}

fn toggle_runtime(state: &AppState) -> WaterGuideSnapshot {
    let current = pipeline::current_payload(state);
    let now_ms = chrono::Utc::now().timestamp_millis();
    let position = position_for_activation(current.as_ref(), now_ms).ok();
    let mut runtime = state.water_guide.lock_safe();
    runtime.toggle_with_position(position, select_freshwater_target)
}

fn publish(app: &AppHandle, snapshot: WaterGuideSnapshot) -> WaterGuideSnapshot {
    if let Some(route) = &snapshot.route {
        log::info!(
            "water guide: requested={} result=route label={} distance_m={:.0}",
            snapshot.requested,
            route.label,
            route.initial_distance_m,
        );
    } else {
        log::info!(
            "water guide: requested={} result={}",
            snapshot.requested,
            snapshot.error_key.as_deref().unwrap_or("off"),
        );
    }
    events::emit_all(app, "water-guide://changed", snapshot.clone());
    snapshot
}

#[tauri::command]
pub fn get_water_guide_state(state: State<'_, AppState>) -> WaterGuideSnapshot {
    state.water_guide.lock_safe().snapshot()
}

#[tauri::command]
pub fn toggle_water_guide(app: AppHandle, state: State<'_, AppState>) -> WaterGuideSnapshot {
    publish(&app, toggle_runtime(&state))
}

pub fn toggle_from_app(app: &AppHandle) -> WaterGuideSnapshot {
    let state = app.state::<AppState>();
    publish(app, toggle_runtime(&state))
}

pub fn is_requested(app: &AppHandle) -> bool {
    app.state::<AppState>().water_guide.lock_safe().requested
}

#[cfg(test)]
mod tests {
    use super::*;
    use overlay_core::Calibration;

    fn target(label: &str, x_cm: f64, y_cm: f64) -> FreshwaterTarget {
        FreshwaterTarget {
            label: label.into(),
            x_cm,
            y_cm,
            mask_px: [10, 20],
            distance_m: 100.0,
        }
    }

    fn position(confirmed_at_ms: i64, in_bounds: bool) -> crate::events::PositionUpdate {
        crate::events::PositionUpdate {
            x_cm: 1_000.0,
            y_cm: 2_000.0,
            z_cm: 0.0,
            px: 1.0,
            py: 2.0,
            heading_deg: None,
            heading_source: None,
            compass_key: None,
            server_facing_deg: None,
            motion_course_deg: None,
            velocity_x_cm_s: None,
            velocity_y_cm_s: None,
            velocity_px_x_s: None,
            velocity_px_y_s: None,
            confirmed_at_ms,
            prediction_horizon_s: 4.0,
            stale_after_s: 12.0,
            relocated: false,
            refreshed_only: false,
            in_bounds,
        }
    }

    fn square_mask(size: u32, min: u32, max: u32) -> image::RgbaImage {
        let mut mask = image::RgbaImage::new(size, size);
        for y in min..=max {
            for x in min..=max {
                mask.put_pixel(x, y, image::Rgba([0, 140, 255, 255]));
            }
        }
        mask
    }

    #[test]
    fn transparent_ocean_is_never_a_candidate() {
        let mask = square_mask(7, 2, 4);

        let candidates = shallow_candidates(&mask);

        assert!(!candidates.is_empty());
        assert!(candidates
            .iter()
            .all(|&(x, y)| mask.get_pixel(x, y).0[3] >= 128));
        assert!(!candidates.contains(&(0, 0)));
    }

    #[test]
    fn boundary_moves_one_pixel_toward_denser_freshwater() {
        let mask = square_mask(7, 2, 4);

        let candidates = shallow_candidates(&mask);

        assert!(candidates.contains(&(3, 3)));
        assert!(!candidates.contains(&(1, 3)));
    }

    #[test]
    fn nearest_candidate_is_chosen_in_world_distance() {
        let calibration = Calibration {
            map_name: "test".into(),
            min_x: 0.0,
            max_x: 10.0,
            min_y: 0.0,
            max_y: 10.0,
            image_width_px: 10,
            image_height_px: 10,
            north_offset_deg: 0.0,
        };

        let selected = nearest_candidate(&[(1, 1), (8, 8)], 1_500.0, 1_200.0, &calibration);

        assert_eq!(selected, Some((1, 1)));
    }

    #[test]
    fn poi_label_does_not_replace_the_mask_destination() {
        let pois = serde_json::json!({
            "map": "Gateway_v0.21.7",
            "layers": {"water": {"items": [
                {"label": "Near Pond", "x": 10_000.0, "y": 20_000.0},
                {"label": "Far Pond", "x": 500_000.0, "y": 500_000.0}
            ]}}
        });

        let label = nearest_water_label(&pois, 11_000.0, 19_000.0).unwrap();

        assert_eq!(label, "Near Pond");
    }

    #[test]
    fn unsupported_map_is_rejected() {
        let pois = serde_json::json!({
            "map": "Gateway_old",
            "layers": {"water": {"items": []}}
        });

        assert_eq!(
            validate_poi_map(&pois),
            Err(WaterGuideError::UnsupportedMap)
        );
    }

    #[test]
    fn mask_dimensions_must_match_islemaps_calibration() {
        assert_eq!(
            validate_mask_dimensions(2_499, 2_500, Calibration::islemaps()),
            Err(WaterGuideError::InvalidFreshwater),
        );
        assert_eq!(
            validate_mask_dimensions(2_500, 2_500, Calibration::islemaps()),
            Ok(()),
        );
    }

    #[test]
    fn missing_water_labels_fail_closed() {
        let pois = serde_json::json!({
            "map": "Gateway_v0.21.7",
            "layers": {"water": {"items": []}}
        });

        assert_eq!(
            nearest_water_label(&pois, 0.0, 0.0),
            Err(WaterGuideError::MissingWaterLabels),
        );
    }

    #[test]
    fn route_endpoints_stay_locked_until_off_then_on() {
        let mut runtime = WaterGuideRuntime::default();

        runtime.toggle_with_position(Some((1_000.0, 2_000.0)), |_, _, _| {
            Ok(target("Lake", 9_000.0, 8_000.0))
        });
        let first = runtime.snapshot().route.unwrap();
        assert_eq!(runtime.snapshot().route.unwrap(), first);

        runtime.toggle_with_position(Some((5_000.0, 6_000.0)), |_, _, _| unreachable!());
        assert!(!runtime.snapshot().requested);

        runtime.toggle_with_position(Some((5_000.0, 6_000.0)), |_, _, _| {
            Ok(target("Lake", 9_000.0, 8_000.0))
        });
        let second = runtime.snapshot().route.unwrap();
        assert_eq!((second.start_x_cm, second.start_y_cm), (5_000.0, 6_000.0));
        assert_eq!((second.target_x_cm, second.target_y_cm), (9_000.0, 8_000.0));
    }

    #[test]
    fn missing_position_keeps_request_visible_but_draws_no_route() {
        let mut runtime = WaterGuideRuntime::default();

        runtime.toggle_with_position(None, |_, _, _| unreachable!());

        assert_eq!(
            runtime.snapshot(),
            WaterGuideSnapshot {
                requested: true,
                route: None,
                error_key: Some("waiting_for_position".into()),
            }
        );
    }

    #[test]
    fn activation_rejects_stale_or_out_of_bounds_position() {
        assert_eq!(
            position_for_activation(Some(&position(69_999, true)), 100_000),
            Err(WaterGuideError::WaitingForPosition),
        );
        assert_eq!(
            position_for_activation(Some(&position(90_000, false)), 100_000),
            Err(WaterGuideError::WaitingForPosition),
        );
        assert_eq!(
            position_for_activation(Some(&position(90_000, true)), 100_000),
            Ok((1_000.0, 2_000.0)),
        );
    }
}
