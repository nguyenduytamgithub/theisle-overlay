//! All #[tauri::command] handlers — the whole IPC surface, mirrored by
//! `src/lib/api.ts` on the frontend.

use overlay_core::{
    bearing_to_compass_key, pixel_to_world, world_to_pixel, Calibration,
};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::events::{TrailPayload, SETTINGS_CHANGED};
use crate::pipeline;
use crate::settings;
use crate::state::AppState;
use crate::store::{self, Waypoint};

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Value {
    state.settings.lock().unwrap().clone()
}

/// Deep-merge a partial patch into the settings, persist (debounced), and
/// broadcast the full new settings to every window. Shared by the IPC command
/// and the hotkey actions so both paths behave identically.
pub fn apply_settings_patch(app: &AppHandle, patch: Value) -> Value {
    let state = app.state::<AppState>();
    let merged = {
        let mut s = state.settings.lock().unwrap();
        *s = settings::merge(&s, &patch);
        s.clone()
    };
    state.request_settings_save();
    if let Err(e) = app.emit(SETTINGS_CHANGED, merged.clone()) {
        log::warn!("emit settings failed: {e}");
    }
    merged
}

#[tauri::command]
pub fn patch_settings(app: AppHandle, patch: Value) -> Value {
    apply_settings_patch(&app, patch)
}

/// Settings-screen probe: is this key combination valid AND currently free?
/// Registering on a scratch id and immediately unregistering answers both.
#[tauri::command]
pub fn check_hotkey_available(spec: String) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
    };
    const PROBE_ID: i32 = 0x3FFF;
    let Some((mods, vk)) = crate::hotkeys::parse_hotkey(&spec) else {
        return false;
    };
    unsafe {
        if RegisterHotKey(None, PROBE_ID, HOT_KEY_MODIFIERS(mods), vk).is_ok() {
            let _ = UnregisterHotKey(None, PROBE_ID);
            true
        } else {
            false
        }
    }
}

/// Re-register all hotkeys from the current settings (after a rebind).
#[tauri::command]
pub fn apply_hotkeys(app: AppHandle, state: State<AppState>) {
    state.hotkeys.restart(app.clone());
}

#[tauri::command]
pub fn list_waypoints(state: State<AppState>) -> Vec<Waypoint> {
    state.waypoints.lock().unwrap().clone()
}

#[derive(Serialize)]
pub struct WaypointPx {
    #[serde(flatten)]
    pub waypoint: Waypoint,
    pub px: f64,
    pub py: f64,
}

/// Waypoints with render pixels attached — the transform stays in Rust.
#[tauri::command]
pub fn list_waypoints_px(state: State<AppState>) -> Vec<WaypointPx> {
    let cal = Calibration::gateway();
    state
        .waypoints
        .lock()
        .unwrap()
        .iter()
        .map(|wp| {
            let (px, py) = world_to_pixel(wp.x, wp.y, cal);
            WaypointPx {
                waypoint: wp.clone(),
                px,
                py,
            }
        })
        .collect()
}

fn persist_waypoints(waypoints: &[Waypoint]) {
    if let Err(e) = store::save_waypoints(waypoints) {
        log::warn!("saving waypoints failed: {e}");
    }
}

/// Right-click on the full map: the frontend sends the clicked PIXEL and Rust
/// converts — the transform stays single-sourced. Stored coords are raw cm.
#[tauri::command]
pub fn add_waypoint_at_pixel(state: State<AppState>, px: f64, py: f64, name: String) -> Waypoint {
    let (x, y) = pixel_to_world(px, py, Calibration::gateway());
    let wp = store::new_waypoint(&name, x, y, 0.0, None);
    let mut waypoints = state.waypoints.lock().unwrap();
    waypoints.push(wp.clone());
    persist_waypoints(&waypoints);
    wp
}

/// The "mark here" hotkey action: drop a waypoint at the current position.
#[tauri::command]
pub fn add_waypoint_here(state: State<AppState>, name: String) -> Option<Waypoint> {
    let current = state.tracker.lock().unwrap().current?;
    let wp = store::new_waypoint(&name, current.x, current.y, current.z, None);
    let mut waypoints = state.waypoints.lock().unwrap();
    waypoints.push(wp.clone());
    persist_waypoints(&waypoints);
    Some(wp)
}

#[tauri::command]
pub fn rename_waypoint(state: State<AppState>, id: String, name: String) -> bool {
    let mut waypoints = state.waypoints.lock().unwrap();
    let Some(wp) = waypoints.iter_mut().find(|w| w.id == id) else {
        return false;
    };
    wp.name = name;
    persist_waypoints(&waypoints);
    true
}

#[tauri::command]
pub fn delete_waypoint(state: State<AppState>, id: String) -> bool {
    let mut waypoints = state.waypoints.lock().unwrap();
    let before = waypoints.len();
    waypoints.retain(|w| w.id != id);
    let removed = waypoints.len() != before;
    if removed {
        persist_waypoints(&waypoints);
    }
    removed
}

/// The previous session's trail (bug fix: the old app wrote trails but never
/// restored them), rendered dimmed on both maps.
#[tauri::command]
pub fn get_previous_trail(state: State<AppState>) -> TrailPayload {
    match &state.previous_trail_path {
        Some(path) => {
            pipeline::trail_payload(&store::load_trail(path), Calibration::gateway())
        }
        None => TrailPayload::default(),
    }
}

/// The current session's trail so far — for a window opening mid-session.
#[tauri::command]
pub fn get_current_trail(state: State<AppState>) -> TrailPayload {
    let tracker = state.tracker.lock().unwrap();
    pipeline::trail_payload(&tracker.segments, Calibration::gateway())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataStatus {
    pub basemap_minimap: bool,
    pub basemap_fullmap: bool,
    pub pois: bool,
}

#[tauri::command]
pub fn data_status() -> DataStatus {
    DataStatus {
        basemap_minimap: settings::basemap_dir().join("minimap.webp").exists(),
        basemap_fullmap: settings::basemap_dir().join("fullmap.webp").exists(),
        pois: settings::pois_path().exists(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BasemapPaths {
    pub minimap: String,
    pub fullmap: String,
}

/// Absolute paths for the frontend to feed through `convertFileSrc()` (asset
/// protocol) — the images are never bundled into the app.
#[tauri::command]
pub fn get_basemap_paths() -> BasemapPaths {
    BasemapPaths {
        minimap: settings::basemap_dir()
            .join("minimap.webp")
            .to_string_lossy()
            .into_owned(),
        fullmap: settings::basemap_dir()
            .join("fullmap.webp")
            .to_string_lossy()
            .into_owned(),
    }
}

/// Raw pois_gateway.json (already px+cm normalised by the fetch step).
#[tauri::command]
pub fn get_pois() -> Result<Value, String> {
    let text = std::fs::read_to_string(settings::pois_path()).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoiItem {
    pub label: String,
    pub px: f64,
    pub py: f64,
    /// cm, so the minimap can distance-filter without any transform.
    pub x_cm: f64,
    pub y_cm: f64,
    /// Circle zones: radius in basemap pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius_px: Option<f64>,
    /// Polygon zones: vertices in basemap pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub points_px: Option<Vec<(f64, f64)>>,
    /// Zones: where to place the name label (polygon centroid, circle
    /// centre) — computed here so the frontend never does geometry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_py: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoiLayer {
    pub key: String,
    /// "point" | "zone"
    pub kind: String,
    pub items: Vec<PoiItem>,
}

/// POI layers with every coordinate already converted to basemap pixels —
/// the frontend renders, it never transforms.
#[tauri::command]
pub fn get_pois_render() -> Result<Vec<PoiLayer>, String> {
    let cal = Calibration::gateway();
    let text = std::fs::read_to_string(settings::pois_path()).map_err(|e| e.to_string())?;
    let raw: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let Some(layers) = raw.get("layers").and_then(|l| l.as_object()) else {
        return Ok(Vec::new());
    };

    let mut out = Vec::new();
    for (key, layer) in layers {
        let kind = layer
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("point")
            .to_string();
        let mut items = Vec::new();
        for item in layer
            .get("items")
            .and_then(|i| i.as_array())
            .unwrap_or(&Vec::new())
        {
            let (Some(x), Some(y)) = (
                item.get("x").and_then(|v| v.as_f64()),
                item.get("y").and_then(|v| v.as_f64()),
            ) else {
                continue;
            };
            let (px, py) = world_to_pixel(x, y, cal);
            let shape = item.get("shape").and_then(|s| s.as_str());
            // Same metres->basemap-pixels factor the original layers.py used.
            let radius_px = (shape == Some("circle"))
                .then(|| item.get("radius_m").and_then(|r| r.as_f64()))
                .flatten()
                .map(|r_m| r_m * 100.0 / 1000.0 / cal.span_y() * cal.image_width_px as f64)
                .filter(|r| *r > 0.0);
            let points_px = (shape == Some("polygon"))
                .then(|| item.get("points").and_then(|p| p.as_array()))
                .flatten()
                .map(|pts| {
                    pts.iter()
                        .filter_map(|p| {
                            let x = p.get(0)?.as_f64()?;
                            let y = p.get(1)?.as_f64()?;
                            Some(world_to_pixel(x, y, cal))
                        })
                        .collect::<Vec<_>>()
                })
                .filter(|pts: &Vec<_>| pts.len() >= 3);
            let (label_px, label_py) = if kind == "zone" {
                match &points_px {
                    // Vertex centroid is plenty for name placement.
                    Some(pts) => {
                        let n = pts.len() as f64;
                        (
                            Some(pts.iter().map(|p| p.0).sum::<f64>() / n),
                            Some(pts.iter().map(|p| p.1).sum::<f64>() / n),
                        )
                    }
                    None => (Some(px), Some(py)),
                }
            } else {
                (None, None)
            };
            items.push(PoiItem {
                label: item
                    .get("label")
                    .and_then(|l| l.as_str())
                    .unwrap_or_default()
                    .to_string(),
                px,
                py,
                x_cm: x,
                y_cm: y,
                radius_px,
                points_px,
                label_px,
                label_py,
            });
        }
        out.push(PoiLayer {
            key: key.clone(),
            kind,
            items,
        });
    }
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NearestWaypoint {
    pub id: String,
    pub name: String,
    pub bearing_deg: f64,
    pub compass_key: &'static str,
    pub distance_m: f64,
}

/// Closest saved waypoint to the current position, with bearing — geometry
/// stays in Rust like every other transform.
#[tauri::command]
pub fn nearest_waypoint(state: State<AppState>) -> Option<NearestWaypoint> {
    let tracker = state.tracker.lock().unwrap();
    let waypoints = state.waypoints.lock().unwrap();
    let mut best: Option<NearestWaypoint> = None;
    for wp in waypoints.iter() {
        let Some((bearing, dist)) = tracker.bearing_to(wp.x, wp.y) else {
            return None; // no current position yet
        };
        if best.as_ref().is_none_or(|b| dist < b.distance_m) {
            best = Some(NearestWaypoint {
                id: wp.id.clone(),
                name: wp.name.clone(),
                bearing_deg: bearing,
                compass_key: bearing_to_compass_key(bearing),
                distance_m: dist,
            });
        }
    }
    best
}

/// 0 = exclusive fullscreen (overlay cannot draw) -> the UI shows a warning
/// banner. None = game config not found.
#[tauri::command]
pub fn get_fullscreen_mode() -> Option<i32> {
    settings::read_game_fullscreen_mode()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapInfo {
    pub image_width_px: u32,
    pub image_height_px: u32,
    /// Basemap pixels per real-world metre, horizontal / vertical.
    pub px_per_m_x: f64,
    pub px_per_m_y: f64,
}

/// Scale constants the minimap needs for its radius maths — derived from the
/// calibration in Rust so the frontend holds no transform of its own.
#[tauri::command]
pub fn get_map_info() -> MapInfo {
    let cal = Calibration::gateway();
    MapInfo {
        image_width_px: cal.image_width_px,
        image_height_px: cal.image_height_px,
        px_per_m_x: cal.image_width_px as f64 / (cal.span_y() * 10.0),
        px_per_m_y: cal.image_height_px as f64 / (cal.span_x() * 10.0),
    }
}

/// Start the first-run / re-download data fetch on a worker thread. Progress
/// arrives as `fetch://progress` events, completion as `fetch://finished`.
#[tauri::command]
pub fn fetch_data(app: AppHandle, force: bool) {
    std::thread::spawn(move || {
        crate::fetch::run(&app, force);
    });
}

/// Open the trails folder in Explorer (legacy-compatible path under
/// %APPDATA%\TheIsleOverlay).
#[tauri::command]
pub fn open_trails_folder(app: AppHandle) -> Result<(), String> {
    let dir = settings::trails_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Open the IslePilot login window; completion arrives as dino:// events.
/// MUST be async: building a webview window inside a synchronous command is
/// a documented deadlock/blank-window hazard on Windows.
#[tauri::command]
pub async fn islepilot_login(app: AppHandle, domain: String) -> Result<(), String> {
    crate::islepilot::start_login(&app, domain)
}

#[tauri::command]
pub fn islepilot_cancel_login(app: AppHandle) {
    crate::islepilot::cancel_login(&app);
}

/// Manual fallback: validate + store a pasted Cookie header.
#[tauri::command]
pub async fn islepilot_set_cookie(
    app: AppHandle,
    domain: String,
    cookie: String,
) -> Result<(), String> {
    // Blocking HTTP validation happens off the async runtime's core threads.
    tauri::async_runtime::spawn_blocking(move || {
        crate::islepilot::manual_cookie(&app, domain, cookie)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn islepilot_logout(app: AppHandle) -> Result<(), String> {
    crate::islepilot::logout(&app)
}

/// Re-read islepilot settings and (re)start/stop the poller accordingly —
/// the Dino tab calls this after toggling enabled/interval/map-position.
#[tauri::command]
pub fn islepilot_apply(app: AppHandle) {
    crate::islepilot::restart_poller(&app);
}

#[tauri::command]
pub fn islepilot_state(app: AppHandle) -> crate::islepilot::IslepilotState {
    crate::islepilot::current_state(&app)
}

/// Dev-only: feed a fake sample through the real pipeline.
#[cfg(debug_assertions)]
#[tauri::command]
pub fn simulate_position(app: AppHandle, x: f64, y: f64, z: f64) {
    pipeline::ingest_sample(&app, x, y, z);
}
