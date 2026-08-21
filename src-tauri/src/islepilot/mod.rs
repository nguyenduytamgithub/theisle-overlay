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

pub mod cookies;
pub mod parser;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use overlay_core::{pixel_to_world, Calibration};

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
/// True while a login window is open and being watched. Cleared the moment
/// the user closes that window, so the UI never sits on "waiting for login".
static LOGIN_ACTIVE: AtomicBool = AtomicBool::new(false);

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
    pub last_update: Option<DinoUpdate>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
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
    domain: String,
    interval_s: f64,
    use_map_position: bool,
}

fn read_config(app: &AppHandle) -> PollConfig {
    let state = app.state::<AppState>();
    let s = state.settings.lock_safe();
    PollConfig {
        enabled: settings::get_bool(&s, &["islepilot", "enabled"], false),
        domain: settings::get_str(&s, &["islepilot", "domain"], "").to_string(),
        interval_s: settings::get_f64(&s, &["islepilot", "poll_interval_s"], 10.0)
            .max(MIN_INTERVAL_S),
        use_map_position: settings::get_bool(&s, &["islepilot", "use_map_position"], false),
    }
}

pub fn current_state(app: &AppHandle) -> IslepilotState {
    let config = read_config(app);
    IslepilotState {
        logged_in: !config.domain.is_empty() && cookies::get(&config.domain).is_some(),
        last_update: LAST_UPDATE.lock_safe().clone(),
    }
}

fn publish(app: &AppHandle, update: DinoUpdate) {
    *LAST_UPDATE.lock_safe() = Some(update.clone());
    crate::events::emit_all(app, DINO_UPDATE, update);
}

/// Re-send the latest update — part of resync after a webview reload.
pub fn emit_last(app: &AppHandle) {
    if let Some(update) = LAST_UPDATE.lock_safe().clone() {
        crate::events::emit_all(app, DINO_UPDATE, update);
    }
}

/// Feed the panel's own live-map position (percent of map frame) into the
/// normal one-way position pipeline. Assumes the panel frames the exact
/// calibration bounds; logged loudly so a mismatch is diagnosable.
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
    if !config.enabled || config.domain.is_empty() {
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
                    let player = parser::parse_me(&html);
                    if !player.looks_logged_in() {
                        // A logged-out page and a site-markup change look
                        // identical here (every stat parses to None), and
                        // killing the thread over it forced an app restart.
                        // Warn once, keep polling at the backed-off rate — a
                        // later successful parse or re-login self-heals.
                        if !auth_warned {
                            auth_warned = true;
                            let _ = app.emit(DINO_AUTH_EXPIRED, config.domain.clone());
                        }
                        failures = failures.saturating_add(1);
                    } else {
                        auth_warned = false;
                        failures = 0;
                        // Fetch /map when position use is on, or once as a
                        // capability probe — the availability answer drives
                        // the use_map_position setting (sync_map_pref) and
                        // the checkbox state in the UI.
                        let map = if config.use_map_position || live_map.is_none() {
                            match get_page(&client, &config.domain, "/map", &cookie) {
                                Ok(map_html) => {
                                    let map = parser::parse_map(&map_html);
                                    let available = !map.map_disabled;
                                    if live_map != Some(available) {
                                        live_map = Some(available);
                                        sync_map_pref(&app, available);
                                    }
                                    if config.use_map_position {
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

/// Exponential backoff for consecutive poll failures, capped at 5 minutes:
/// a long outage costs one request per 5 min and recovery stays automatic.
pub(crate) fn backoff_s(base: f64, failures: u32) -> f64 {
    (base * 2f64.powi(failures.min(6) as i32)).min(300.0)
}

pub fn stop_poller() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    *LAST_UPDATE.lock_safe() = None;
}

/// Open the panel in a login window; once the user finishes Steam sign-in
/// there, grab the session cookies from the webview, verify them against
/// /me, store them (DPAPI) and start polling.
pub fn start_login(app: &AppHandle, domain: String) -> Result<(), String> {
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
            if parser::parse_me(&html).looks_logged_in() {
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
    if !parser::parse_me(&html).looks_logged_in() {
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

pub fn logout(app: &AppHandle) -> Result<(), String> {
    let config = read_config(app);
    stop_poller();
    if !config.domain.is_empty() {
        cookies::remove(&config.domain)?;
    }
    crate::commands::apply_settings_patch(
        app,
        serde_json::json!({ "islepilot": { "enabled": false } }),
    );
    Ok(())
}
