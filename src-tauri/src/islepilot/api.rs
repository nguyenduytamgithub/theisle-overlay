//! JSON client for the CENTRAL IslePilot overlay API (`islepilot.eu`).
//!
//! Unlike the per-server HTML panels (parser.rs), this API authenticates with
//! ONE bearer overlay-token that follows the player across every IslePilot
//! server — the backend itself knows which server they are on. Endpoints and
//! headers were verified against the official overlay app (see
//! rv/TheIsleVN-Gacha-HUD-integration-guide.md).

use serde::Deserialize;
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

}
