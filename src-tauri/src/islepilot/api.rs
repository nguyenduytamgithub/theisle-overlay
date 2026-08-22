//! JSON client for the CENTRAL IslePilot overlay API (`islepilot.eu`).
//!
//! Unlike the per-server HTML panels (parser.rs), this API authenticates with
//! ONE bearer overlay-token that follows the player across every IslePilot
//! server — the backend itself knows which server they are on. Endpoints and
//! headers were verified against the official overlay app (see
//! rv/TheIsleVN-Gacha-HUD-integration-guide.md).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::parser::{Nutrition, PlayerStats, QuestStatus, StatBar};

pub const API_ORIGIN: &str = "https://islepilot.eu";

#[derive(Debug)]
pub enum ApiError {
    /// 401 / `{"error":"unauthorized"}` — token expired or revoked.
    Unauthorized,
    /// 404 — account has never been on an IslePilot server. Not a failure.
    NotFound,
    Http(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Unauthorized => write!(f, "unauthorized"),
            ApiError::NotFound => write!(f, "not found"),
            ApiError::Http(e) => write!(f, "{e}"),
        }
    }
}

fn request(
    client: &reqwest::blocking::Client,
    method: reqwest::Method,
    path: &str,
    token: &str,
    body: Option<&Value>,
) -> Result<Value, ApiError> {
    let mut req = client
        .request(method, format!("{API_ORIGIN}{path}"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .header("X-Overlay-Version", "2");
    if let Some(body) = body {
        // reqwest's `json` feature is off in this crate — set the body by hand.
        req = req
            .header("Content-Type", "application/json")
            .body(body.to_string());
    }
    let resp = req.send().map_err(|e| ApiError::Http(e.to_string()))?;
    let status = resp.status();
    let text = resp.text().map_err(|e| ApiError::Http(e.to_string()))?;
    if status.as_u16() == 401 {
        return Err(ApiError::Unauthorized);
    }
    if status.as_u16() == 404 {
        return Err(ApiError::NotFound);
    }
    if !status.is_success() {
        return Err(ApiError::Http(format!("{path} -> HTTP {status}")));
    }
    let v: Value =
        serde_json::from_str(&text).map_err(|e| ApiError::Http(format!("{path}: {e}")))?;
    // Some auth failures come back 200 with an error body.
    if v.get("error").and_then(|e| e.as_str()) == Some("unauthorized") {
        return Err(ApiError::Unauthorized);
    }
    Ok(v)
}

fn get(
    client: &reqwest::blocking::Client,
    path: &str,
    token: &str,
) -> Result<Value, ApiError> {
    request(client, reqwest::Method::GET, path, token, None)
}

fn post(
    client: &reqwest::blocking::Client,
    path: &str,
    token: &str,
    body: &Value,
) -> Result<Value, ApiError> {
    request(client, reqwest::Method::POST, path, token, Some(body))
}

// ---------------------------------------------------------------------------
// /api/overlay/me — vitals + position + prime progress
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OverlayMe {
    pub has_data: bool,
    pub steam_id: Option<String>,
    pub name: Option<String>,
    pub server: Option<String>,
    pub online: Option<bool>,
    pub species: Option<String>,
    pub female: Option<bool>,
    pub growth: Option<f64>,
    pub health: Option<f64>,
    pub max_health: Option<f64>,
    pub hunger: Option<f64>,
    pub max_hunger: Option<f64>,
    pub thirst: Option<f64>,
    pub max_thirst: Option<f64>,
    pub stamina: Option<f64>,
    pub max_stamina: Option<f64>,
    pub nutrition: Option<OverlayNutrition>,
    pub position: Option<OverlayPosition>,
    pub prime: Option<OverlayPrime>,
}

#[derive(Deserialize, Debug, Clone, Copy, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OverlayNutrition {
    pub carb: f64,
    pub protein: f64,
    pub lipid: f64,
}

#[derive(Deserialize, Debug, Clone, Copy, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OverlayPosition {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
    pub yaw: Option<f64>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OverlayPrime {
    pub eligible: bool,
    pub elder: bool,
    pub quests: Vec<OverlayQuest>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct OverlayQuest {
    pub name: String,
    pub done: bool,
}

pub fn get_me(
    client: &reqwest::blocking::Client,
    token: &str,
) -> Result<OverlayMe, ApiError> {
    let v = get(client, "/api/overlay/me", token)?;
    serde_json::from_value(v).map_err(|e| ApiError::Http(format!("/api/overlay/me: {e}")))
}

/// Map the JSON vitals into the exact struct the HTML parser produces, so the
/// whole downstream (DinoTab, minimap panels, translate) is untouched.
pub fn to_player_stats(me: &OverlayMe) -> PlayerStats {
    // Observed as a 0..1 fraction (0.2628); tolerate an already-percent value
    // defensively.
    let growth_pct = me.growth.map(|g| if g <= 1.5 { g * 100.0 } else { g });
    let bar = |cur: Option<f64>, max: Option<f64>| -> Option<StatBar> {
        Some(StatBar::from_values(cur?, max?))
    };
    PlayerStats {
        dino_name: me.species.clone(),
        online: me.online,
        growth: growth_pct.map(|p| format!("{}%", p.round() as i64)),
        growth_pct,
        health: bar(me.health, me.max_health),
        hunger: bar(me.hunger, me.max_hunger),
        thirst: bar(me.thirst, me.max_thirst),
        prime_quests: me
            .prime
            .as_ref()
            .map(|p| {
                p.quests
                    .iter()
                    .map(|q| QuestStatus {
                        text: q.name.clone(),
                        text_vi: None,
                        completed: q.done,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        stamina: bar(me.stamina, me.max_stamina),
        nutrition: me.nutrition.map(|n| Nutrition {
            carb: n.carb,
            protein: n.protein,
            lipid: n.lipid,
        }),
        server: me.server.clone(),
        female: me.female,
    }
}

/// Own position in game cm, OUR axis convention (their x = our y — the same
/// swap `parse_own_marker` uses, verified against named landmarks).
pub fn position_cm(me: &OverlayMe) -> Option<(f64, f64)> {
    let pos = me.position?;
    Some((pos.y?, pos.x?))
}

// ---------------------------------------------------------------------------
// /api/overlay/map — POIs + categories (token mode extra map layers)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OverlayMap {
    pub live_map_enabled: Option<bool>,
    pub allowed: Option<bool>,
    pub calibration: Option<OverlayCalibration>,
    pub pois: Vec<OverlayPoi>,
    pub categories: Vec<OverlayCategory>,
}

#[derive(Deserialize, Debug, Clone, Copy, Default)]
#[serde(default)]
pub struct OverlayCalibration {
    pub a: OverlayCalPoint,
    pub b: OverlayCalPoint,
}

#[derive(Deserialize, Debug, Clone, Copy, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OverlayCalPoint {
    pub u: f64,
    pub v: f64,
    pub world_x: f64,
    pub world_y: f64,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OverlayPoi {
    pub id: Option<String>,
    pub name: Option<String>,
    pub category_id: Option<String>,
    pub color: Option<String>,
    pub shape: Option<String>,
    pub size: Option<f64>,
    pub points: Vec<OverlayPoint>,
}

#[derive(Deserialize, Debug, Clone, Copy, Default)]
#[serde(default)]
pub struct OverlayPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OverlayCategory {
    pub id: Option<String>,
    pub name: Option<String>,
    pub color: Option<String>,
}

pub fn get_map(
    client: &reqwest::blocking::Client,
    token: &str,
) -> Result<OverlayMap, ApiError> {
    let v = get(client, "/api/overlay/map", token)?;
    serde_json::from_value(v).map_err(|e| ApiError::Http(format!("/api/overlay/map: {e}")))
}

impl OverlayCalibration {
    /// Their-map (u,v 0..1) -> world (their axes). Two independent linear
    /// interpolations, exactly what the official app does.
    fn uv_to_world(&self, u: f64, v: f64) -> Option<(f64, f64)> {
        let (a, b) = (self.a, self.b);
        if (b.u - a.u).abs() < f64::EPSILON || (b.v - a.v).abs() < f64::EPSILON {
            return None;
        }
        let world_x = a.world_x + (u - a.u) / (b.u - a.u) * (b.world_x - a.world_x);
        let world_y = a.world_y + (v - a.v) / (b.v - a.v) * (b.world_y - a.world_y);
        Some((world_x, world_y))
    }
}

/// One POI point in game cm, OUR axis convention. POI points have been seen
/// both as u,v fractions and as raw world cm depending on backend version, so
/// disambiguate by magnitude: |coord| <= 1.5 can only be a fraction (1.5 cm
/// off the world origin is not a real POI).
pub fn poi_point_cm(
    cal: Option<&OverlayCalibration>,
    p: OverlayPoint,
) -> Option<(f64, f64)> {
    let (their_x, their_y) = if p.x.abs() <= 1.5 && p.y.abs() <= 1.5 {
        cal?.uv_to_world(p.x, p.y)?
    } else {
        (p.x, p.y)
    };
    Some((their_y, their_x)) // their x = our y
}

// ---------------------------------------------------------------------------
// /api/overlay/garage — gacha park/restore/sell/rename
// ---------------------------------------------------------------------------

pub fn garage_list(
    client: &reqwest::blocking::Client,
    token: &str,
) -> Result<Value, ApiError> {
    get(client, "/api/overlay/garage", token)
}

/// POST a garage command and, when it is asynchronous (`commandId` in the
/// response), poll its status to completion: 1.5 s x 40 tries (~60 s), the
/// exact pattern the official app uses.
pub fn garage_command(
    client: &reqwest::blocking::Client,
    token: &str,
    path: &str,
    body: Value,
) -> Result<Value, String> {
    let res = post(client, path, token, &body).map_err(|e| e.to_string())?;
    if let Some(err) = res.get("error").and_then(|e| e.as_str()) {
        return Err(err.to_string());
    }
    let Some(command_id) = res.get("commandId").and_then(|c| c.as_str()).map(String::from)
    else {
        return Ok(res); // synchronous command (e.g. rename, sell)
    };
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(1500));
        let s = get(
            client,
            &format!("/api/overlay/garage/status?id={command_id}"),
            token,
        )
        .map_err(|e| e.to_string())?;
        match s.get("status").and_then(|st| st.as_str()) {
            Some("done") => return Ok(s),
            Some("failed") => {
                return Err(s
                    .get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("failed")
                    .to_string())
            }
            _ => {} // pending — keep waiting
        }
    }
    Err("timeout".to_string())
}

/// Serialized state for the frontend garage panel.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GarageState {
    pub dinos: Value,
    pub selling_enabled: bool,
    pub live_swap: bool,
    pub currency_name: Option<String>,
}

pub fn garage_state(raw: &Value) -> GarageState {
    let settings = raw.get("settings").cloned().unwrap_or(Value::Null);
    GarageState {
        dinos: raw.get("dinos").cloned().unwrap_or_else(|| Value::Array(vec![])),
        selling_enabled: settings
            .get("sellingEnabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        live_swap: settings
            .get("liveSwap")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        currency_name: settings
            .get("currencyName")
            .and_then(|v| v.as_str())
            .map(String::from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = include_str!("../../fixtures/islepilot/overlay_me.json");
    const ME_NODATA: &str = include_str!("../../fixtures/islepilot/overlay_me_nodata.json");

    #[test]
    fn overlay_me_maps_to_player_stats() {
        let me: OverlayMe = serde_json::from_str(ME).unwrap();
        assert!(me.has_data);
        let stats = to_player_stats(&me);
        assert_eq!(stats.dino_name.as_deref(), Some("Tyrannosaurus"));
        assert_eq!(stats.online, Some(true));
        assert_eq!(stats.growth.as_deref(), Some("26%"));
        assert!((stats.growth_pct.unwrap() - 26.28).abs() < 0.01);
        let health = stats.health.as_ref().unwrap();
        assert_eq!((health.current, health.max), (Some(49.01), Some(55.12)));
        assert_eq!(health.raw, "49 / 55.1");
        assert_eq!(stats.server.as_deref(), Some("PVN The Isle Viet Nam 01"));
        assert_eq!(stats.female, Some(false));
        let stamina = stats.stamina.as_ref().unwrap();
        assert_eq!(stamina.max, Some(336.52));
        let nut = stats.nutrition.unwrap();
        assert!((nut.carb - 4.04).abs() < 0.001);
        assert_eq!(stats.prime_quests.len(), 2);
        assert_eq!(
            stats.prime_quests[0].text,
            "Visit a Sanctuary as a juvenile"
        );
        assert!(!stats.prime_quests[0].completed);
        assert!(stats.prime_quests[1].completed);
        assert!(stats.looks_logged_in());
    }

    #[test]
    fn position_axis_swap_matches_markers_convention() {
        let me: OverlayMe = serde_json::from_str(ME).unwrap();
        // JSON: x=-263306, y=307415.69 -> ours (x=their y, y=their x).
        assert_eq!(position_cm(&me), Some((307415.69, -263306.0)));
    }

    #[test]
    fn no_data_response_is_not_an_error() {
        let me: OverlayMe = serde_json::from_str(ME_NODATA).unwrap();
        assert!(!me.has_data);
        assert_eq!(position_cm(&me), None);
        let stats = to_player_stats(&me);
        assert!(!stats.looks_logged_in());
    }

    #[test]
    fn poi_points_convert_from_both_spaces() {
        let cal = OverlayCalibration {
            a: OverlayCalPoint { u: 0.0, v: 0.0, world_x: -100_000.0, world_y: -200_000.0 },
            b: OverlayCalPoint { u: 1.0, v: 1.0, world_x: 100_000.0, world_y: 200_000.0 },
        };
        // uv fraction: center of the map -> world origin -> swapped ours.
        assert_eq!(
            poi_point_cm(Some(&cal), OverlayPoint { x: 0.5, y: 0.5 }),
            Some((0.0, 0.0))
        );
        // Raw world cm passes through (with the axis swap).
        assert_eq!(
            poi_point_cm(Some(&cal), OverlayPoint { x: 50_000.0, y: -30_000.0 }),
            Some((-30_000.0, 50_000.0))
        );
        // Fraction without calibration: unusable.
        assert_eq!(poi_point_cm(None, OverlayPoint { x: 0.5, y: 0.5 }), None);
    }

    #[test]
    fn garage_state_reads_settings_flags() {
        let raw: Value = serde_json::json!({
            "dinos": [{"id": "d1", "species": "Carnotaurus"}],
            "settings": {"liveSwap": true, "sellingEnabled": false, "currencyName": "Points"}
        });
        let g = garage_state(&raw);
        assert!(g.live_swap);
        assert!(!g.selling_enabled);
        assert_eq!(g.currency_name.as_deref(), Some("Points"));
        assert_eq!(g.dinos.as_array().unwrap().len(), 1);
    }
}
