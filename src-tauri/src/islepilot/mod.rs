//! "Your dino" — IslePilot server-panel integration.
//!
//! Reads the player's OWN dino stats (growth/health/hunger/thirst, Prime
//! progress) and optionally their own live-map position from the server's
//! companion website (e.g. mixi.islepilot.eu). Pure HTTPS to a public panel
//! the admin runs — the game process is never touched, so the EAC safety
//! boundary is unaffected.
//!
//! Login: a normal webview window is opened on the panel; the user signs in
//! with Steam there, and the session cookie is read back through WebView2's
//! native cookie manager (`cookies_for_url`, includes httpOnly), then stored
//! DPAPI-encrypted. No manual devtools cookie copying.

pub mod api;
pub mod cookies;
pub mod parser;
pub mod token;

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use overlay_core::{pixel_to_world, world_to_pixel, Calibration};

use crate::pipeline;
use crate::settings;
use crate::state::{AppState, LockExt};

use parser::{MapPosition, PlayerStats};

pub const DINO_UPDATE: &str = "dino://update";
pub const DINO_AUTH_EXPIRED: &str = "dino://auth-expired";
pub const DINO_LOGIN_OK: &str = "dino://login-ok";
pub const DINO_LOGIN_FAILED: &str = "dino://login-failed";

const LOGIN_WINDOW: &str = "islepilot-login";
const MIN_INTERVAL_S: f64 = 5.0;
const BUILD_ID_CHECK_S: f64 = 600.0;

/// Poller generation: bumping it makes any running poll loop exit on its
/// next tick. This is how login/logout/settings changes restart cleanly.
static GENERATION: AtomicU64 = AtomicU64::new(0);
static LAST_UPDATE: Mutex<Option<DinoUpdate>> = Mutex::new(None);
/// Prime-quest count of the last GOOD update (error publishes keep the
/// previous value so a network hiccup can't collapse the overlay panel).
/// Read by minimap::snapshot each supervisor tick to size the window.
static QUEST_COUNT: AtomicUsize = AtomicUsize::new(0);
/// Whether the last GOOD update carried a stamina bar (token mode only) —
/// the minimap stats strip grows one row for it.
static HAS_STAMINA: AtomicBool = AtomicBool::new(false);
/// True while a login window is open and being watched. Cleared the moment
/// the user closes that window, so the UI never sits on "waiting for login".
static LOGIN_ACTIVE: AtomicBool = AtomicBool::new(false);
/// (steamId, overlayToken) captured by the token-login window's navigation
/// hook; consumed by its watcher thread.
static CAPTURED_TOKEN: Mutex<Option<(String, String)>> = Mutex::new(None);

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DinoUpdate {
    pub domain: String,
    pub fetched_at_ms: u64,
    pub player: Option<PlayerStats>,
    pub map: Option<MapPosition>,
    /// IslePilot deployed a new build since we started — markup may have
    /// changed, so treat odd values with suspicion.
    pub layout_changed: bool,
    /// Whether this server runs a live map at all (probed from /map).
    /// None until the first successful probe.
    pub live_map_available: Option<bool>,
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IslepilotState {
    pub logged_in: bool,
    /// "token" (central overlay API, one login for every server) or
    /// "legacy" (per-server cookie).
    pub auth_mode: String,
    pub token_present: bool,
    pub last_update: Option<DinoUpdate>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("theisle-overlay/2.0 (your-dino panel reader; personal use)")
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())
}

fn get_page(client: &reqwest::blocking::Client, domain: &str, path: &str, cookie: &str) -> Result<String, String> {
    let url = format!("{}{}", domain.trim_end_matches('/'), path);
    let resp = client
        .get(&url)
        .header("Cookie", cookie)
        .send()
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("GET {path} -> HTTP {status}"));
    }
    Ok(body)
}

/// Server slug for the JSON API (`/api/p/{slug}/...`):
/// "https://sdvn2.islepilot.eu" -> "sdvn2", "https://islepilot.eu/p/name" ->
/// "name". None -> the HTML fallback carries the feature alone.
fn server_slug(domain: &str) -> Option<String> {
    let url: tauri::Url = domain.trim_end_matches('/').parse().ok()?;
    if let Some(segs) = url.path_segments() {
        let segs: Vec<_> = segs.filter(|s| !s.is_empty()).collect();
        if segs.len() >= 2 && segs[0] == "p" {
            return Some(segs[1].to_string());
        }
    }
    // Subdomain form: first label of a host with at least 3 labels.
    let host = url.host_str()?;
    let label = host.split('.').next()?;
    if host.matches('.').count() >= 2 && label != "www" {
        return Some(label.to_string());
    }
    None
}

/// The API lives at the ORIGIN root even for path-form panels
/// (islepilot.eu/p/name -> https://islepilot.eu/api/p/name/...).
fn origin_of(domain: &str) -> Option<String> {
    let url: tauri::Url = domain.trim_end_matches('/').parse().ok()?;
    let host = url.host_str()?;
    Some(match url.port() {
        Some(p) => format!("{}://{}:{}", url.scheme(), host, p),
        None => format!("{}://{}", url.scheme(), host),
    })
}

/// Minimal base64url decoder — enough to read our own JWT payload without a
/// new dependency (no verification: we are the client reading our own token).
fn b64url_decode(s: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut idx = [255u8; 256];
    for (i, &c) in TABLE.iter().enumerate() {
        idx[c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0u32);
    for &c in s.as_bytes() {
        if c == b'=' {
            break;
        }
        let v = idx[c as usize];
        if v == 255 {
            return None;
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// Our steamId out of the islepilot_player session JWT — the robust way to
/// pick "our" marker out of the markers list (group members can appear too).
fn steam_id_from_cookie(cookie: &str) -> Option<String> {
    let token = cookie.split(';').find_map(|kv| {
        let (k, v) = kv.trim().split_once('=')?;
        (k == "islepilot_player").then(|| v.to_string())
    })?;
    let payload = token.split('.').nth(1)?;
    let json = String::from_utf8(b64url_decode(payload)?).ok()?;
    serde_json::from_str::<serde_json::Value>(&json)
        .ok()?
        .get("steamId")?
        .as_str()
        .map(String::from)
}

/// Own position from the markers JSON, converted to OUR axis convention.
///
/// AXIS SWAP — verified against named landmarks in the panel's own map SVG
/// (Mudflats/South Plains/Swamp/EastLake all agree): islepilot `x` is game
/// Long (our y, horizontal), islepilot `y` is game Lat (our x, vertical) —
/// the same swap myislemap uses. Values are raw UE cm.
///
/// Ok(None) = endpoint answered but carries no usable position (`ok:false`,
/// empty list, ...) — the caller falls back to the HTML page.
fn parse_own_marker(body: &str, own_steam_id: Option<&str>) -> Result<Option<(f64, f64)>, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    if v.get("ok").and_then(|b| b.as_bool()) != Some(true) {
        return Ok(None);
    }
    let markers = v
        .get("markers")
        .and_then(|m| m.as_array())
        .ok_or("no markers array")?;
    let own = markers
        .iter()
        .find(|m| {
            own_steam_id.is_some() && m.get("steamId").and_then(|s| s.as_str()) == own_steam_id
        })
        .or_else(|| {
            markers
                .iter()
                .find(|m| m.get("label").and_then(|l| l.as_str()) == Some("You"))
        })
        .or_else(|| if markers.len() == 1 { markers.first() } else { None });
    let Some(m) = own else { return Ok(None) };
    let (Some(long_cm), Some(lat_cm)) = (
        m.get("x").and_then(|x| x.as_f64()),
        m.get("y").and_then(|y| y.as_f64()),
    ) else {
        return Ok(None);
    };
    Ok(Some((lat_cm, long_cm)))
}

/// GET /api/p/{slug}/map/markers and extract our own position (game cm, our
/// axis convention).
fn fetch_own_marker(
    client: &reqwest::blocking::Client,
    origin: &str,
    slug: &str,
    cookie: &str,
    own_steam_id: Option<&str>,
) -> Result<Option<(f64, f64)>, String> {
    let body = get_page(client, origin, &format!("/api/p/{slug}/map/markers"), cookie)?;
    parse_own_marker(&body, own_steam_id)
}

fn build_id(client: &reqwest::blocking::Client, domain: &str) -> Option<String> {
    let url = format!("{}/api/version", domain.trim_end_matches('/'));
    let body = client.get(&url).send().ok()?.text().ok()?;
    serde_json::from_str::<serde_json::Value>(&body)
        .ok()?
        .get("buildId")?
        .as_str()
        .map(String::from)
}

struct PollConfig {
    enabled: bool,
    auth_mode: String,
    domain: String,
    interval_s: f64,
    use_map_position: bool,
}

fn read_config(app: &AppHandle) -> PollConfig {
    let state = app.state::<AppState>();
    let s = state.settings.lock_safe();
    PollConfig {
        enabled: settings::get_bool(&s, &["islepilot", "enabled"], false),
        auth_mode: settings::get_str(&s, &["islepilot", "auth_mode"], "legacy").to_string(),
        domain: settings::get_str(&s, &["islepilot", "domain"], "").to_string(),
        interval_s: settings::get_f64(&s, &["islepilot", "poll_interval_s"], 10.0)
            .max(MIN_INTERVAL_S),
        use_map_position: settings::get_bool(&s, &["islepilot", "use_map_position"], false),
    }
}

pub fn current_state(app: &AppHandle) -> IslepilotState {
    let config = read_config(app);
    let token_present = token::get().is_some();
    let logged_in = if config.auth_mode == "token" {
        token_present
    } else {
        !config.domain.is_empty() && cookies::get(&config.domain).is_some()
    };
    IslepilotState {
        logged_in,
        auth_mode: config.auth_mode,
        token_present,
        last_update: LAST_UPDATE.lock_safe().clone(),
    }
}

fn publish(app: &AppHandle, update: DinoUpdate) {
    if let Some(player) = &update.player {
        QUEST_COUNT.store(player.prime_quests.len(), Ordering::SeqCst);
        HAS_STAMINA.store(player.stamina.is_some(), Ordering::SeqCst);
    }
    *LAST_UPDATE.lock_safe() = Some(update.clone());
    crate::events::emit_all(app, DINO_UPDATE, update);
}

/// Quest count of the last good update — the minimap window's quests panel
/// is sized from this.
pub fn last_quest_count() -> usize {
    QUEST_COUNT.load(Ordering::SeqCst)
}

/// Whether the last good update had stamina — the minimap stats strip is
/// sized from this (one extra row in token mode).
pub fn last_has_stamina() -> bool {
    HAS_STAMINA.load(Ordering::SeqCst)
}

/// Re-send the latest update — part of resync after a webview reload.
pub fn emit_last(app: &AppHandle) {
    if let Some(update) = LAST_UPDATE.lock_safe().clone() {
        crate::events::emit_all(app, DINO_UPDATE, update);
    }
}

/// Feed the panel's own live-map position (percent of map frame) into the
/// normal one-way position pipeline. Assumes the panel frames the exact
/// VULNONA calibration bounds; logged loudly so a mismatch is diagnosable.
///
/// PINNED to Vulnona deliberately: the pct is relative to the SERVER PANEL's
/// map frame — a property of that site, independent of which basemap the user
/// is viewing. The cm we produce here flows through `pipeline::ingest_sample`,
/// which converts cm -> px with the ACTIVE calibration, so display is correct
/// on any basemap. Do not switch this to `state.active_calibration()`.
fn ingest_map_position(app: &AppHandle, map: &MapPosition) {
    let (Some(pct_x), Some(pct_y)) = (map.pct_x, map.pct_y) else {
        return;
    };
    let cal = Calibration::gateway();
    let px = pct_x / 100.0 * cal.image_width_px as f64;
    let py = pct_y / 100.0 * cal.image_height_px as f64;
    let (x_cm, y_cm) = pixel_to_world(px, py, cal);
    log::debug!("islepilot position: {pct_x:.2}%,{pct_y:.2}% -> {x_cm:.0},{y_cm:.0} cm");
    pipeline::ingest_sample(app, x_cm, y_cm, 0.0);
}

/// Keep `use_map_position` truthful to the server's capability: no live map
/// -> force it off (the UI disables the checkbox); live map present ->
/// default it ON, unless the user has ever flipped the toggle themselves
/// (`map_pref_user_set`). The poller re-reads settings every iteration, so
/// no restart is needed after the patch.
fn sync_map_pref(app: &AppHandle, available: bool) {
    let state = app.state::<AppState>();
    let (use_map, user_set) = {
        let s = state.settings.lock_safe();
        (
            settings::get_bool(&s, &["islepilot", "use_map_position"], false),
            settings::get_bool(&s, &["islepilot", "map_pref_user_set"], false),
        )
    };
    let desired = if !available {
        false
    } else if !user_set {
        true
    } else {
        use_map
    };
    if desired != use_map {
        log::info!(
            "islepilot live map {} -> use_map_position={desired}",
            if available { "available" } else { "disabled" }
        );
        crate::commands::apply_settings_patch(
            app,
            serde_json::json!({ "islepilot": { "use_map_position": desired } }),
        );
    }
}

/// (Re)start the background poller from current settings. Safe to call any
/// time; the previous loop exits on its next tick.
pub fn restart_poller(app: &AppHandle) {
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let config = read_config(app);
    if !config.enabled {
        return;
    }
    if config.auth_mode == "token" {
        let Some(tok) = token::get() else {
            return; // not logged in yet — the login flow will restart us
        };
        run_token_poll(app.clone(), generation, tok);
        return;
    }
    if config.domain.is_empty() {
        return;
    }
    let Some(cookie) = cookies::get(&config.domain) else {
        return; // not logged in yet — the login flow will restart us
    };
    let app = app.clone();
    std::thread::spawn(move || {
        // A failed client build must not silently kill the poller for the
        // whole session — retry until superseded.
        let client = loop {
            if GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            match http_client() {
                Ok(c) => break c,
                Err(e) => {
                    log::warn!("islepilot http client: {e}");
                    std::thread::sleep(Duration::from_secs(5));
                }
            }
        };
        let initial_build = build_id(&client, &config.domain);
        let mut layout_changed = false;
        let mut last_build_check = std::time::Instant::now();
        let mut failures: u32 = 0;
        let mut auth_warned = false;
        // Probed lazily from /map (even when position use is off) so the UI
        // can tell the user up front whether this server has a live map.
        let mut live_map: Option<bool> = None;
        // Our steamId (from the session JWT) picks "our" marker out of the
        // markers API response; None just weakens the match to label=="You".
        let own_steam_id = steam_id_from_cookie(&cookie);

        loop {
            if GENERATION.load(Ordering::SeqCst) != generation {
                return; // superseded
            }
            let config = read_config(&app);
            if !config.enabled {
                return;
            }

            if last_build_check.elapsed().as_secs_f64() > BUILD_ID_CHECK_S && !layout_changed {
                last_build_check = std::time::Instant::now();
                if let (Some(a), Some(b)) = (&initial_build, build_id(&client, &config.domain)) {
                    if *a != b {
                        layout_changed = true;
                    }
                }
            }

            match get_page(&client, &config.domain, "/me", &cookie) {
                Ok(html) => {
                    let mut player = parser::parse_me(&html);
                    // Missing stats alone is NOT auth loss: a logged-in
                    // player with no living dino parses to all-None too.
                    // Only a page without the session markers counts — and
                    // even then (it could be a site-markup change) warn
                    // once and keep polling at the backed-off rate; a later
                    // good parse or re-login self-heals.
                    if !player.looks_logged_in() && !parser::looks_authenticated(&html) {
                        if !auth_warned {
                            auth_warned = true;
                            let _ = app.emit(DINO_AUTH_EXPIRED, config.domain.clone());
                        }
                        failures = failures.saturating_add(1);
                    } else {
                        auth_warned = false;
                        failures = 0;
                        // Vietnamese quest text (dict -> templates -> cache ->
                        // budgeted MyMemory fallback). English UI skips it —
                        // no reason to spend the free API tier; switching the
                        // language picks it up on the next tick.
                        let lang_vi = {
                            let state = app.state::<AppState>();
                            let s = state.settings.lock_safe();
                            settings::get_str(&s, &["language"], "vi") != "en"
                        };
                        if lang_vi {
                            crate::translate::translate_quests(&mut player.prime_quests, &client);
                        }
                        // Position source 1 — the JSON markers API (the same
                        // one the panel's own 15-second poll uses): exact UE
                        // cm, no pct->px->cm roundtrip, immune to markup
                        // changes. Any miss falls through to the HTML page.
                        let mut position_from_api = false;
                        if config.use_map_position {
                            if let (Some(origin), Some(slug)) =
                                (origin_of(&config.domain), server_slug(&config.domain))
                            {
                                match fetch_own_marker(
                                    &client,
                                    &origin,
                                    &slug,
                                    &cookie,
                                    own_steam_id.as_deref(),
                                ) {
                                    Ok(Some((x_cm, y_cm))) => {
                                        position_from_api = true;
                                        if live_map != Some(true) {
                                            live_map = Some(true);
                                            sync_map_pref(&app, true);
                                        }
                                        log::debug!(
                                            "islepilot markers api: {x_cm:.0},{y_cm:.0} cm"
                                        );
                                        pipeline::ingest_sample(&app, x_cm, y_cm, 0.0);
                                    }
                                    // ok:false / no own marker: map may be
                                    // off — let the HTML probe decide.
                                    Ok(None) => {}
                                    Err(e) => {
                                        log::warn!("islepilot markers api failed: {e}");
                                    }
                                }
                            }
                        }
                        // Position source 2 / capability probe — the HTML
                        // page. Fetched when the API produced nothing while
                        // position use is on, or once as the availability
                        // probe that drives the use_map_position setting
                        // (sync_map_pref) and the checkbox state in the UI.
                        let need_html =
                            (config.use_map_position && !position_from_api) || live_map.is_none();
                        let map = if need_html {
                            match get_page(&client, &config.domain, "/map", &cookie) {
                                Ok(map_html) => {
                                    let map = parser::parse_map(&map_html);
                                    let available = !map.map_disabled;
                                    if live_map != Some(available) {
                                        live_map = Some(available);
                                        sync_map_pref(&app, available);
                                    }
                                    if config.use_map_position && !position_from_api {
                                        ingest_map_position(&app, &map);
                                    }
                                    Some(map)
                                }
                                Err(e) => {
                                    log::warn!("islepilot /map fetch failed: {e}");
                                    None
                                }
                            }
                        } else {
                            None
                        };
                        publish(
                            &app,
                            DinoUpdate {
                                domain: config.domain.clone(),
                                fetched_at_ms: now_ms(),
                                player: Some(player),
                                map,
                                layout_changed,
                                live_map_available: live_map,
                                error: None,
                            },
                        );
                    }
                }
                Err(e) => {
                    failures = failures.saturating_add(1);
                    // Network hiccup: report but keep polling.
                    publish(
                        &app,
                        DinoUpdate {
                            domain: config.domain.clone(),
                            fetched_at_ms: now_ms(),
                            player: None,
                            map: None,
                            layout_changed,
                            live_map_available: live_map,
                            error: Some(e),
                        },
                    );
                }
            }

            // Sleep in short slices so a generation bump stops us promptly.
            let mut remaining = backoff_s(config.interval_s.max(MIN_INTERVAL_S), failures);
            while remaining > 0.0 {
                if GENERATION.load(Ordering::SeqCst) != generation {
                    return;
                }
                std::thread::sleep(Duration::from_millis(500));
                remaining -= 0.5;
            }
        }
    });
}

/// Token-mode poll loop: the CENTRAL overlay API (`islepilot.eu`), one token
/// for every server — no domain, no HTML, no buildId watching. Same
/// generation/backoff skeleton as the cookie loop.
fn run_token_poll(app: AppHandle, generation: u64, tok: token::OverlayToken) {
    std::thread::spawn(move || {
        let client = loop {
            if GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            match http_client() {
                Ok(c) => break c,
                Err(e) => {
                    log::warn!("islepilot http client: {e}");
                    std::thread::sleep(Duration::from_secs(5));
                }
            }
        };
        let mut failures: u32 = 0;
        let mut auth_warned = false;
        let mut live_map: Option<bool> = None;

        loop {
            if GENERATION.load(Ordering::SeqCst) != generation {
                return; // superseded
            }
            let config = read_config(&app);
            if !config.enabled || config.auth_mode != "token" {
                return;
            }

            match api::get_me(&client, &tok.token) {
                Ok(me) => {
                    auth_warned = false;
                    failures = 0;
                    let mut player = api::to_player_stats(&me);
                    let lang_vi = {
                        let state = app.state::<AppState>();
                        let s = state.settings.lock_safe();
                        settings::get_str(&s, &["language"], "vi") != "en"
                    };
                    if lang_vi && !player.prime_quests.is_empty() {
                        crate::translate::translate_quests(&mut player.prime_quests, &client);
                    }
                    let position = api::position_cm(&me);
                    // Position availability doubles as the live-map probe.
                    // Only trust it while the API actually has data.
                    if me.has_data {
                        let available = position.is_some();
                        if live_map != Some(available) {
                            live_map = Some(available);
                            sync_map_pref(&app, available);
                        }
                    }
                    // Never move the marker from cached (offline) data.
                    if config.use_map_position && me.online == Some(true) {
                        if let Some((x_cm, y_cm)) = position {
                            log::debug!("islepilot overlay api: {x_cm:.0},{y_cm:.0} cm");
                            pipeline::ingest_sample(&app, x_cm, y_cm, 0.0);
                        }
                    }
                    publish(
                        &app,
                        DinoUpdate {
                            domain: api::API_ORIGIN.to_string(),
                            fetched_at_ms: now_ms(),
                            player: Some(player),
                            map: None,
                            layout_changed: false,
                            live_map_available: live_map,
                            error: None,
                        },
                    );
                }
                // Never been on an IslePilot server: an empty-stats update,
                // NOT an error (mirrors the cookie flow's "No dino" page).
                Err(api::ApiError::NotFound) => {
                    auth_warned = false;
                    failures = 0;
                    publish(
                        &app,
                        DinoUpdate {
                            domain: api::API_ORIGIN.to_string(),
                            fetched_at_ms: now_ms(),
                            player: Some(api::to_player_stats(&api::OverlayMe::default())),
                            map: None,
                            layout_changed: false,
                            live_map_available: live_map,
                            error: None,
                        },
                    );
                }
                Err(api::ApiError::Unauthorized) => {
                    failures = failures.saturating_add(1);
                    if !auth_warned {
                        auth_warned = true;
                        let _ = app.emit(DINO_AUTH_EXPIRED, api::API_ORIGIN.to_string());
                    }
                }
                Err(api::ApiError::Http(e)) => {
                    failures = failures.saturating_add(1);
                    publish(
                        &app,
                        DinoUpdate {
                            domain: api::API_ORIGIN.to_string(),
                            fetched_at_ms: now_ms(),
                            player: None,
                            map: None,
                            layout_changed: false,
                            live_map_available: live_map,
                            error: Some(e),
                        },
                    );
                }
            }

            let mut remaining = backoff_s(config.interval_s.max(MIN_INTERVAL_S), failures);
            while remaining > 0.0 {
                if GENERATION.load(Ordering::SeqCst) != generation {
                    return;
                }
                std::thread::sleep(Duration::from_millis(500));
                remaining -= 0.5;
            }
        }
    });
}

/// Exponential backoff for consecutive poll failures, capped at 5 minutes:
/// a long outage costs one request per 5 min and recovery stays automatic.
pub(crate) fn backoff_s(base: f64, failures: u32) -> f64 {
    (base * 2f64.powi(failures.min(6) as i32)).min(300.0)
}

pub fn stop_poller() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    *LAST_UPDATE.lock_safe() = None;
    QUEST_COUNT.store(0, Ordering::SeqCst);
}

/// Open the panel in a login window; once the user finishes Steam sign-in
/// there, grab the session cookies from the webview, verify them against
/// /me, store them (DPAPI) and start polling.
pub fn start_login(app: &AppHandle, domain: String) -> Result<(), String> {
    // Same normalization as manual_cookie: the domain string is the cookie
    // store's key, so a trailing slash must not create a second identity.
    let domain = domain.trim().trim_end_matches('/').to_string();
    let url: tauri::Url = domain.parse().map_err(|e| format!("URL không hợp lệ: {e}"))?;
    if url.scheme() != "https" {
        return Err("Domain phải bắt đầu bằng https://".into());
    }

    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = existing.set_focus();
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(app, LOGIN_WINDOW, WebviewUrl::External(url.clone()))
        .title(url.host_str().unwrap_or("IslePilot"))
        .inner_size(520.0, 760.0)
        .build()
        .map_err(|e| e.to_string())?;

    LOGIN_ACTIVE.store(true, Ordering::SeqCst);

    // Closing the window must end the wait IMMEDIATELY — polling for the
    // window's disappearance was too slow and could miss it entirely.
    let close_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
        ) && LOGIN_ACTIVE.swap(false, Ordering::SeqCst)
        {
            let _ = close_app.emit(DINO_LOGIN_FAILED, "cancelled");
        }
    });

    let app = app.clone();
    std::thread::spawn(move || {
        let Ok(client) = http_client() else { return };
        // ~3 minutes at 2 s per check.
        for _ in 0..90 {
            std::thread::sleep(Duration::from_secs(2));
            if !LOGIN_ACTIVE.load(Ordering::SeqCst) {
                return; // window closed or cancelled from the UI
            }
            let Some(window) = app.get_webview_window(LOGIN_WINDOW) else {
                if LOGIN_ACTIVE.swap(false, Ordering::SeqCst) {
                    let _ = app.emit(DINO_LOGIN_FAILED, "cancelled");
                }
                return;
            };
            let Ok(cookie_list) = window.cookies_for_url(url.clone()) else {
                continue;
            };
            if cookie_list.is_empty() {
                continue;
            }
            let header = cookie_list
                .iter()
                .map(|c| format!("{}={}", c.name(), c.value()))
                .collect::<Vec<_>>()
                .join("; ");
            let Ok(html) = get_page(&client, &domain, "/me", &header) else {
                continue;
            };
            // Session check, not a stats check — a fresh account with no
            // dino must still be able to log in.
            if parser::looks_authenticated(&html) {
                if let Err(e) = cookies::set(&domain, &header) {
                    log::warn!("saving islepilot cookie failed: {e}");
                }
                // Claim the flag first so closing the window does not fire
                // the "cancelled" path over a successful login.
                LOGIN_ACTIVE.store(false, Ordering::SeqCst);
                let _ = window.close();
                // Logging in implies the user wants the feature on.
                crate::commands::apply_settings_patch(
                    &app,
                    serde_json::json!({ "islepilot": { "enabled": true, "domain": domain } }),
                );
                let _ = app.emit(DINO_LOGIN_OK, domain.clone());
                restart_poller(&app);
                return;
            }
        }
        if LOGIN_ACTIVE.swap(false, Ordering::SeqCst) {
            if let Some(window) = app.get_webview_window(LOGIN_WINDOW) {
                let _ = window.close();
            }
            let _ = app.emit(DINO_LOGIN_FAILED, "timeout");
        }
    });

    Ok(())
}

/// One-time Steam login against the CENTRAL overlay API. The backend
/// redirects to `isle-overlay://?sid=..&token=..` after Steam sign-in; we
/// intercept that navigation INSIDE our login webview (`on_navigation`
/// returning false) instead of registering the protocol system-wide — so the
/// official overlay app's handler, if installed, is never hijacked.
pub fn start_token_login(app: &AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = existing.set_focus();
        return Ok(());
    }
    let url: tauri::Url = format!("{}/api/overlay/auth/steam", api::API_ORIGIN)
        .parse()
        .map_err(|e| format!("URL: {e}"))?;
    *CAPTURED_TOKEN.lock_safe() = None;

    let window = WebviewWindowBuilder::new(app, LOGIN_WINDOW, WebviewUrl::External(url))
        .title("IslePilot — Steam")
        .inner_size(520.0, 760.0)
        .on_navigation(|nav_url| {
            if nav_url.scheme() == "isle-overlay" {
                let (mut sid, mut tok) = (None, None);
                for (k, v) in nav_url.query_pairs() {
                    match k.as_ref() {
                        "sid" => sid = Some(v.to_string()),
                        "token" => tok = Some(v.to_string()),
                        _ => {}
                    }
                }
                if let (Some(sid), Some(tok)) = (sid, tok) {
                    *CAPTURED_TOKEN.lock_safe() = Some((sid, tok));
                }
                // NOTE: if some WebView2 build ever stops surfacing custom-
                // scheme navigations here, the timeout + manual token paste
                // is the escape hatch.
                return false; // never let the OS resolve the custom scheme
            }
            true
        })
        .build()
        .map_err(|e| e.to_string())?;

    LOGIN_ACTIVE.store(true, Ordering::SeqCst);

    let close_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
        ) && LOGIN_ACTIVE.swap(false, Ordering::SeqCst)
        {
            let _ = close_app.emit(DINO_LOGIN_FAILED, "cancelled");
        }
    });

    let app = app.clone();
    std::thread::spawn(move || {
        let Ok(client) = http_client() else { return };
        // ~3 minutes at 500 ms per check.
        for _ in 0..360 {
            std::thread::sleep(Duration::from_millis(500));
            if !LOGIN_ACTIVE.load(Ordering::SeqCst) {
                return; // window closed or cancelled from the UI
            }
            if app.get_webview_window(LOGIN_WINDOW).is_none() {
                if LOGIN_ACTIVE.swap(false, Ordering::SeqCst) {
                    let _ = app.emit(DINO_LOGIN_FAILED, "cancelled");
                }
                return;
            }
            let Some((sid, tok)) = CAPTURED_TOKEN.lock_safe().take() else {
                continue;
            };
            match api::get_me(&client, &tok) {
                // A fresh account (never on an IslePilot server) is 404 —
                // still a perfectly valid login.
                Ok(_) | Err(api::ApiError::NotFound) => {
                    if let Err(e) = token::set(&token::OverlayToken {
                        token: tok,
                        steam_id: sid,
                    }) {
                        log::warn!("saving overlay token failed: {e}");
                    }
                    // Claim the flag first so closing the window does not
                    // fire the "cancelled" path over a successful login.
                    LOGIN_ACTIVE.store(false, Ordering::SeqCst);
                    if let Some(window) = app.get_webview_window(LOGIN_WINDOW) {
                        let _ = window.close();
                    }
                    crate::commands::apply_settings_patch(
                        &app,
                        serde_json::json!({
                            "islepilot": { "enabled": true, "auth_mode": "token" }
                        }),
                    );
                    let _ = app.emit(DINO_LOGIN_OK, api::API_ORIGIN.to_string());
                    restart_poller(&app);
                    return;
                }
                Err(api::ApiError::Unauthorized) => {
                    LOGIN_ACTIVE.store(false, Ordering::SeqCst);
                    if let Some(window) = app.get_webview_window(LOGIN_WINDOW) {
                        let _ = window.close();
                    }
                    let _ = app.emit(DINO_LOGIN_FAILED, "invalid-token");
                    return;
                }
                Err(e) => {
                    // Transient network failure validating: keep the capture
                    // and retry on the next tick.
                    log::warn!("overlay token validation failed: {e}");
                    *CAPTURED_TOKEN.lock_safe() = Some((sid, tok));
                }
            }
        }
        if LOGIN_ACTIVE.swap(false, Ordering::SeqCst) {
            if let Some(window) = app.get_webview_window(LOGIN_WINDOW) {
                let _ = window.close();
            }
            let _ = app.emit(DINO_LOGIN_FAILED, "timeout");
        }
    });

    Ok(())
}

/// Manual fallback for token mode: paste either the bare overlay token or
/// the whole `isle-overlay://?sid=..&token=..` redirect URL. Validated
/// against the API before being stored.
pub fn manual_token(app: &AppHandle, raw: String) -> Result<(), String> {
    let raw = raw.trim().trim_matches('"').trim().to_string();
    if raw.is_empty() {
        return Err("invalid-token".into());
    }
    let (mut sid, tok) = if raw.contains("token=") {
        let url: tauri::Url = raw.parse().map_err(|_| "invalid-token".to_string())?;
        let (mut sid, mut tok) = (String::new(), None);
        for (k, v) in url.query_pairs() {
            match k.as_ref() {
                "sid" => sid = v.to_string(),
                "token" => tok = Some(v.to_string()),
                _ => {}
            }
        }
        (sid, tok.ok_or_else(|| "invalid-token".to_string())?)
    } else {
        (String::new(), raw)
    };
    let client = http_client()?;
    match api::get_me(&client, &tok) {
        Ok(me) => {
            if sid.is_empty() {
                sid = me.steam_id.unwrap_or_default();
            }
        }
        Err(api::ApiError::NotFound) => {} // valid token, fresh account
        Err(api::ApiError::Unauthorized) => return Err("invalid-token".into()),
        Err(e) => return Err(e.to_string()),
    }
    token::set(&token::OverlayToken { token: tok, steam_id: sid })?;
    crate::commands::apply_settings_patch(
        app,
        serde_json::json!({ "islepilot": { "enabled": true, "auth_mode": "token" } }),
    );
    let _ = app.emit(DINO_LOGIN_OK, api::API_ORIGIN.to_string());
    restart_poller(app);
    Ok(())
}

/// UI "cancel" button: stop waiting and close the login window if it is
/// still around.
pub fn cancel_login(app: &AppHandle) {
    LOGIN_ACTIVE.store(false, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = window.close();
    }
}

/// Manual fallback: the user pastes a Cookie header copied from their
/// browser devtools (the prototype's original flow). Validated against /me
/// before being stored, so a bad paste is rejected with a clear error.
pub fn manual_cookie(app: &AppHandle, domain: String, cookie: String) -> Result<(), String> {
    // Normalize: a trailing slash would silently split the cookie-store key
    // from a later slash-less entry of the same server.
    let domain = domain.trim().trim_end_matches('/').to_string();
    let url: tauri::Url = domain.parse().map_err(|e| format!("URL: {e}"))?;
    if url.scheme() != "https" {
        return Err("invalid-url".into());
    }
    // Accept either a full Cookie header ("a=1; b=2") or just the bare
    // islepilot_player VALUE, which is what devtools' "Value" column gives.
    let raw = cookie.trim().trim_matches('"').trim_matches(';').trim();
    if raw.is_empty() {
        return Err("invalid-cookie".into());
    }
    // A real header starts with a cookie NAME before the first '=';
    // a bare JWT value has no '=' before its first '.' (it is base64url,
    // whose padding, if any, only appears at the end).
    let looks_like_header = raw
        .split_once('=')
        .is_some_and(|(name, _)| {
            !name.is_empty()
                && name.len() < 64
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        });
    let cookie = if looks_like_header {
        raw.to_string()
    } else {
        format!("islepilot_player={raw}")
    };
    let client = http_client()?;
    let html = get_page(&client, &domain, "/me", &cookie)?;
    // Session check, NOT a stats check: a player with no dino on this
    // server is still logged in (field case: valid cookie rejected because
    // /me said "No dino" and no stat parsed).
    if !parser::looks_authenticated(&html) {
        return Err("invalid-cookie".into());
    }
    cookies::set(&domain, &cookie)?;
    crate::commands::apply_settings_patch(
        app,
        serde_json::json!({ "islepilot": { "enabled": true, "domain": domain } }),
    );
    let _ = app.emit(DINO_LOGIN_OK, domain);
    restart_poller(app);
    Ok(())
}

// ---------------------------------------------------------------------------
// Token-mode extras: overlay-map POIs + garage (gacha)
// ---------------------------------------------------------------------------

/// Raw /api/overlay/map cache — the data changes rarely; 15 s matches the
/// official app's poll and keeps tab switches free.
static OVERLAY_MAP_CACHE: Mutex<Option<(Instant, api::OverlayMap)>> = Mutex::new(None);

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OverlayMapRender {
    pub available: bool,
    /// Why not, when unavailable: "not-logged-in" | "disabled" (operator
    /// turned the live map off) | "discord" (needs a linked Discord) |
    /// "empty".
    pub reason: Option<String>,
    pub categories: Vec<OverlayCategoryOut>,
    pub pois: Vec<OverlayPoiRender>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OverlayCategoryOut {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OverlayPoiRender {
    pub id: String,
    pub name: Option<String>,
    pub category_id: Option<String>,
    pub color: Option<String>,
    pub shape: Option<String>,
    /// Render pixels on the ACTIVE basemap, one per source point.
    pub points_px: Vec<[f64; 2]>,
}

impl OverlayMapRender {
    fn unavailable(reason: &str) -> Self {
        Self {
            available: false,
            reason: Some(reason.to_string()),
            categories: Vec::new(),
            pois: Vec::new(),
        }
    }
}

/// IslePilot POIs (sanctuaries, migration/patrol zones, ...) converted to
/// render pixels for the full map. Token mode only.
pub fn overlay_map_render(app: &AppHandle) -> Result<OverlayMapRender, String> {
    let Some(tok) = token::get() else {
        return Ok(OverlayMapRender::unavailable("not-logged-in"));
    };
    let cached = {
        let cache = OVERLAY_MAP_CACHE.lock_safe();
        cache
            .as_ref()
            .filter(|(at, _)| at.elapsed().as_secs_f64() < 15.0)
            .map(|(_, m)| m.clone())
    };
    let raw = match cached {
        Some(m) => m,
        None => {
            let client = http_client()?;
            match api::get_map(&client, &tok.token) {
                Ok(m) => {
                    *OVERLAY_MAP_CACHE.lock_safe() = Some((Instant::now(), m.clone()));
                    m
                }
                Err(api::ApiError::Unauthorized) => return Err("unauthorized".into()),
                Err(api::ApiError::NotFound) => {
                    return Ok(OverlayMapRender::unavailable("empty"))
                }
                Err(e) => return Err(e.to_string()),
            }
        }
    };
    if raw.live_map_enabled == Some(false) {
        return Ok(OverlayMapRender::unavailable("disabled"));
    }
    if raw.allowed == Some(false) {
        return Ok(OverlayMapRender::unavailable("discord"));
    }
    let state = app.state::<AppState>();
    let cal = state.active_calibration();
    let source_cal = raw.calibration;
    let pois: Vec<OverlayPoiRender> = raw
        .pois
        .iter()
        .filter_map(|poi| {
            let points_px: Vec<[f64; 2]> = poi
                .points
                .iter()
                .filter_map(|&p| {
                    let (x_cm, y_cm) = api::poi_point_cm(source_cal.as_ref(), p)?;
                    let (px, py) = world_to_pixel(x_cm, y_cm, cal);
                    Some([px, py])
                })
                .collect();
            if points_px.is_empty() {
                return None;
            }
            Some(OverlayPoiRender {
                id: poi.id.clone().unwrap_or_default(),
                name: poi.name.clone(),
                category_id: poi.category_id.clone(),
                color: poi.color.clone(),
                shape: poi.shape.clone(),
                points_px,
            })
        })
        .collect();
    if pois.is_empty() {
        return Ok(OverlayMapRender::unavailable("empty"));
    }
    Ok(OverlayMapRender {
        available: true,
        reason: None,
        categories: raw
            .categories
            .iter()
            .filter_map(|c| {
                Some(OverlayCategoryOut {
                    id: c.id.clone()?,
                    name: c.name.clone().unwrap_or_default(),
                    color: c.color.clone(),
                })
            })
            .collect(),
        pois,
    })
}

/// Public skinviewer CDN (3D dino models + textures). No auth; served
/// without CORS headers, which is exactly why the download goes through
/// Rust instead of the webview.
const SKINVIEWER_CDN: &str = "https://islepilot.eu/cdn/skinviewer/";

/// Client for CDN downloads only: models run 3-10 MB and a slow link must
/// NOT be cut mid-download, so unlike `http_client` there is no overall
/// request timeout — just a connect timeout so a dead host still fails fast.
fn cdn_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("theisle-overlay/2.0 (your-dino panel reader; personal use)")
        .connect_timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

/// Download-and-cache one skinviewer asset; returns the local file path
/// (under cache/skinviewer, covered by the asset-protocol scope). Already-
/// cached files are returned without a network roundtrip. Progress streams
/// out as `cdn://progress` events so the 3D viewer can show real numbers
/// instead of an opaque spinner (field report: 10 MB model on ~1 MB/s).
pub fn cdn_asset(app: &AppHandle, url: &str) -> Result<String, String> {
    let rel = url
        .strip_prefix(SKINVIEWER_CDN)
        .ok_or_else(|| "invalid-url".to_string())?;
    // The URL becomes a filesystem path: refuse anything path-traversal-ish.
    if rel.is_empty()
        || rel
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == ".." || seg.contains('\\'))
    {
        return Err("invalid-url".into());
    }
    let mut dest = settings::cache_dir().join("skinviewer");
    for seg in rel.split('/') {
        dest.push(seg);
    }
    if dest.exists() {
        return Ok(dest.to_string_lossy().into_owned());
    }
    let client = cdn_client()?;
    let mut resp = client.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // Stream to a .part file, rename on success — a killed download never
    // leaves a truncated file that would be treated as a cache hit forever.
    // APPEND the suffix (with_extension would REPLACE .glb/.png, letting two
    // same-stem files clobber each other's temp).
    let tmp = dest.with_file_name(format!(
        "{}.part",
        dest.file_name().unwrap_or_default().to_string_lossy()
    ));
    let result = (|| -> Result<(), String> {
        use std::io::{Read, Write};
        let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut buf = [0u8; 64 * 1024];
        let mut received: u64 = 0;
        let mut last_emit: u64 = 0;
        loop {
            let n = resp.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            received += n as u64;
            // ~4 events per MB keeps the UI live without spamming IPC.
            if received - last_emit >= 262_144 {
                last_emit = received;
                crate::events::emit_all(
                    app,
                    "cdn://progress",
                    serde_json::json!({ "url": url, "received": received, "total": total }),
                );
            }
        }
        crate::events::emit_all(
            app,
            "cdn://progress",
            serde_json::json!({ "url": url, "received": received, "total": total }),
        );
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().into_owned())
}

fn token_or_err() -> Result<token::OverlayToken, String> {
    token::get().ok_or_else(|| "not-logged-in".to_string())
}

/// GET the garage (parked dinos + server flags). Token mode only.
pub fn garage_fetch() -> Result<api::GarageState, String> {
    let tok = token_or_err()?;
    let client = http_client()?;
    let raw = api::garage_list(&client, &tok.token).map_err(|e| e.to_string())?;
    Ok(api::garage_state(&raw))
}

/// Run a garage command (park/restore/sell/rename); blocks through the
/// async-command status poll, so call it from spawn_blocking.
pub fn garage_action(path: &str, body: serde_json::Value) -> Result<serde_json::Value, String> {
    let tok = token_or_err()?;
    let client = http_client()?;
    api::garage_command(&client, &tok.token, path, body)
}

/// Log out of the ACTIVE mode only: token mode drops the central token,
/// legacy mode drops the current domain's cookie (others stay stored).
pub fn logout(app: &AppHandle) -> Result<(), String> {
    let config = read_config(app);
    stop_poller();
    if config.auth_mode == "token" {
        token::clear()?;
    } else if !config.domain.is_empty() {
        cookies::remove(&config.domain)?;
    }
    crate::commands::apply_settings_patch(
        app,
        serde_json::json!({ "islepilot": { "enabled": false } }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_and_caps() {
        assert_eq!(backoff_s(10.0, 0), 10.0, "no failures = normal interval");
        assert_eq!(backoff_s(10.0, 1), 20.0);
        assert_eq!(backoff_s(10.0, 2), 40.0);
        assert_eq!(backoff_s(10.0, 5), 300.0, "capped at 5 minutes");
        assert_eq!(backoff_s(10.0, 60), 300.0, "cap sticks, no overflow");
    }

    #[test]
    fn slug_and_origin_for_both_domain_forms() {
        assert_eq!(server_slug("https://sdvn2.islepilot.eu/"), Some("sdvn2".into()));
        assert_eq!(server_slug("https://mixi.islepilot.eu"), Some("mixi".into()));
        assert_eq!(server_slug("https://islepilot.eu/p/myserver"), Some("myserver".into()));
        assert_eq!(server_slug("https://islepilot.eu"), None, "no slug to derive");
        assert_eq!(
            origin_of("https://islepilot.eu/p/myserver/"),
            Some("https://islepilot.eu".into()),
            "API sits at the origin root even for path-form panels"
        );
    }

    #[test]
    fn steam_id_comes_out_of_the_session_jwt() {
        // header.payload.sig with payload {"steamId":"76561198000000001"}
        // (base64url, no padding) — structure matches the real cookie.
        let payload = "eyJzdGVhbUlkIjoiNzY1NjExOTgwMDAwMDAwMDEiLCJwZXJzb25hTmFtZSI6IlgifQ";
        let cookie = format!("other=1; islepilot_player=xxx.{payload}.yyy");
        assert_eq!(
            steam_id_from_cookie(&cookie),
            Some("76561198000000001".to_string())
        );
        assert_eq!(steam_id_from_cookie("islepilot_player=garbage"), None);
        assert_eq!(steam_id_from_cookie("unrelated=1"), None);
    }

    /// Shape captured from a real server (values anonymised). The decisive
    /// assertion is the AXIS SWAP: their x is our y (Long), their y is our
    /// x (Lat) — verified against named landmarks in the panel's map SVG.
    #[test]
    fn own_marker_is_selected_and_axes_are_swapped() {
        let body = r#"{"ok":true,"markers":[
            {"steamId":"111","label":"Friend","x":1.0,"y":2.0},
            {"steamId":"76561198000000001","label":"You","x":-92413.23,"y":38665.41,"yaw":-146.89,
             "path":[{"x":-92413.23,"y":38665.41}]}
        ]}"#;
        let got = parse_own_marker(body, Some("76561198000000001")).unwrap();
        assert_eq!(got, Some((38665.41, -92413.23)), "(game X=their y, game Y=their x)");
        // No steamId available -> the "You" label still finds us.
        assert_eq!(parse_own_marker(body, None).unwrap(), Some((38665.41, -92413.23)));
    }

    #[test]
    fn markers_api_refusals_fall_back_instead_of_erroring() {
        assert_eq!(parse_own_marker(r#"{"ok":false}"#, None).unwrap(), None);
        assert_eq!(
            parse_own_marker(r#"{"ok":true,"markers":[]}"#, Some("1")).unwrap(),
            None
        );
        // Several markers, none provably ours: refuse rather than guess.
        let two = r#"{"ok":true,"markers":[
            {"steamId":"a","label":"P1","x":1.0,"y":2.0},
            {"steamId":"b","label":"P2","x":3.0,"y":4.0}
        ]}"#;
        assert_eq!(parse_own_marker(two, Some("zz")).unwrap(), None);
        assert!(parse_own_marker("not json", None).is_err());
    }

    /// Live check of the markers API through the exact production path
    /// (slug derivation, our client/UA, axis swap):
    ///   THEISLE_TEST_DOMAIN=... THEISLE_TEST_COOKIE=... \
    ///   cargo test -- --ignored live_markers
    #[test]
    #[ignore]
    fn live_markers_api() {
        let (Ok(domain), Ok(cookie)) = (
            std::env::var("THEISLE_TEST_DOMAIN"),
            std::env::var("THEISLE_TEST_COOKIE"),
        ) else {
            eprintln!("set THEISLE_TEST_DOMAIN + THEISLE_TEST_COOKIE to run");
            return;
        };
        let client = http_client().unwrap();
        let origin = origin_of(&domain).expect("origin");
        let slug = server_slug(&domain).expect("slug");
        let own = steam_id_from_cookie(&cookie);
        eprintln!("origin={origin} slug={slug} steamId={own:?}");
        let pos = fetch_own_marker(&client, &origin, &slug, &cookie, own.as_deref())
            .expect("markers api reachable");
        eprintln!("own position (game cm, our axes): {pos:?}");
        if let Some((x, y)) = pos {
            let cal = overlay_core::Calibration::gateway();
            let (px, py) = overlay_core::world_to_pixel(x, y, cal);
            eprintln!("-> vulnona px=({px:.0},{py:.0}) of {}x{}", cal.image_width_px, cal.image_height_px);
        }
    }

    /// Live end-to-end check of the exact HTTP path the app uses (our client,
    /// our UA, a real cookie) — the thing fixtures cannot prove:
    ///   THEISLE_TEST_DOMAIN=https://mixi.islepilot.eu \
    ///   THEISLE_TEST_COOKIE="islepilot_player=..." \
    ///   cargo test -- --ignored live_fetch
    #[test]
    #[ignore]
    fn live_fetch_with_real_cookie() {
        let (Ok(domain), Ok(cookie)) = (
            std::env::var("THEISLE_TEST_DOMAIN"),
            std::env::var("THEISLE_TEST_COOKIE"),
        ) else {
            eprintln!("set THEISLE_TEST_DOMAIN + THEISLE_TEST_COOKIE to run");
            return;
        };
        let client = http_client().unwrap();
        let html = get_page(&client, &domain, "/me", &cookie).expect("GET /me");
        let stats = parser::parse_me(&html);
        println!(
            "{domain} -> {:?} growth={:?} hp={:?} quests={}",
            stats.dino_name,
            stats.growth,
            stats.health.as_ref().map(|h| h.raw.clone()),
            stats.prime_quests.len()
        );
        assert!(stats.looks_logged_in(), "cookie should authenticate");
    }

    /// Live check of the CENTRAL overlay API through the exact production
    /// path (our client/UA, headers, mapping, axis swap):
    ///   THEISLE_TEST_TOKEN=... cargo test -- --ignored live_overlay_me
    /// Verifies the two assumptions fixtures cannot prove: the growth scale
    /// (fraction vs percent) and the position axis swap.
    #[test]
    #[ignore]
    fn live_overlay_me() {
        let Ok(tok) = std::env::var("THEISLE_TEST_TOKEN") else {
            eprintln!("set THEISLE_TEST_TOKEN to run");
            return;
        };
        let client = http_client().unwrap();
        let me = api::get_me(&client, &tok).expect("GET /api/overlay/me");
        eprintln!(
            "hasData={} online={:?} server={:?} species={:?} growth={:?}",
            me.has_data, me.online, me.server, me.species, me.growth
        );
        let stats = api::to_player_stats(&me);
        eprintln!(
            "mapped: growth={:?} hp={:?} quests={}",
            stats.growth,
            stats.health.as_ref().map(|h| h.raw.clone()),
            stats.prime_quests.len()
        );
        if let Some((x, y)) = api::position_cm(&me) {
            let cal = overlay_core::Calibration::gateway();
            let (px, py) = overlay_core::world_to_pixel(x, y, cal);
            eprintln!(
                "position (our axes): {x:.0},{y:.0} cm -> vulnona px=({px:.0},{py:.0}) of {}x{}",
                cal.image_width_px, cal.image_height_px
            );
        }
        assert!(me.has_data, "token should resolve to an account with data");
    }

    /// Dev helper: seed the DPAPI cookie store exactly like the UI's paste
    /// flow, to exercise the poller without clicking through the UI.
    ///   THEISLE_TEST_DOMAIN=... THEISLE_TEST_COOKIE=... \
    ///   cargo test -- --ignored seed_cookie
    #[test]
    #[ignore]
    fn seed_cookie() {
        let (Ok(domain), Ok(cookie)) = (
            std::env::var("THEISLE_TEST_DOMAIN"),
            std::env::var("THEISLE_TEST_COOKIE"),
        ) else {
            return;
        };
        cookies::set(&domain, &cookie).expect("store cookie");
        assert_eq!(cookies::get(&domain).as_deref(), Some(cookie.as_str()));
        println!("cookie stored for {domain}");
    }
}

