//! Transparent heading/navigation HUD anchored to the top-centre of The Isle.
//! It reads only the overlay's confirmed position pipeline and never opens or
//! inspects the game process.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Listener, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

use crate::settings::{self, GAME_PROCESS_NAME};
use crate::state::{AppState, LockExt};
use crate::win::{game_window, overlay, vis};

const HUD_WIDTH: f64 = 720.0;
const HUD_HEIGHT: f64 = 104.0;
const HUD_TOP_MARGIN: f64 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadyAction {
    Recreate,
    StartSupervisor,
    None,
}

#[derive(Default)]
struct ReadyRecovery {
    timeouts: u8,
}

impl ReadyRecovery {
    fn on_timeout(&mut self) -> ReadyAction {
        let action = match self.timeouts {
            0 => ReadyAction::Recreate,
            1 => ReadyAction::StartSupervisor,
            _ => ReadyAction::None,
        };
        self.timeouts = self.timeouts.saturating_add(1);
        action
    }
}

fn build_window(app: &AppHandle) -> tauri::Result<tauri::WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, "hud", WebviewUrl::App("hud.html".into()))
        .title("navigation hud")
        .inner_size(HUD_WIDTH, HUD_HEIGHT)
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(false)
        .build()?;

    if let Ok(hwnd) = window.hwnd() {
        let raw = hwnd.0 as isize;
        vis::register("hud", raw);
        overlay::assert_overlay_styles(raw);
    }
    let _ = window.set_ignore_cursor_events(true);
    Ok(window)
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let app_handle = app.clone();
    let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ready_guard = started.clone();
    app.listen_any("hud://ready", move |_| {
        if ready_guard.swap(true, Ordering::SeqCst) {
            return;
        }
        spawn_supervisor(app_handle.clone());
    });

    // Register before creating the webview: the HUD bundle is tiny enough to
    // emit its ready event while `build_window` is still returning.
    build_window(app)?;

    let fallback_app = app.clone();
    std::thread::spawn(move || {
        let mut recovery = ReadyRecovery::default();
        loop {
            std::thread::sleep(Duration::from_secs(5));
            if started.load(Ordering::SeqCst) {
                return;
            }
            match recovery.on_timeout() {
                ReadyAction::Recreate => {
                    log::warn!("hud ready timeout: recreating the webview once");
                    if let Some(window) = fallback_app.get_webview_window("hud") {
                        let _ = window.destroy();
                    }
                    if let Err(error) = build_window(&fallback_app) {
                        log::warn!("hud one-shot recreation failed: {error}");
                    }
                }
                ReadyAction::StartSupervisor => {
                    if !started.swap(true, Ordering::SeqCst) {
                        log::warn!("hud ready timeout after one recreation: starting supervisor");
                        spawn_supervisor(fallback_app);
                    }
                    return;
                }
                ReadyAction::None => return,
            }
        }
    });
    Ok(())
}

#[derive(Clone, Copy, PartialEq)]
struct Snapshot {
    visible: bool,
    game_rect_ms: u64,
    topmost_ms: u64,
}

fn snapshot(app: &AppHandle) -> Snapshot {
    let state = app.state::<AppState>();
    let settings = state.settings.lock_safe();
    Snapshot {
        visible: settings::get_bool(&settings, &["navigation", "hud_visible"], true),
        game_rect_ms: settings::get_f64(&settings, &["poll", "game_rect_ms"], 1000.0) as u64,
        topmost_ms: settings::get_f64(&settings, &["poll", "topmost_ms"], 2000.0) as u64,
    }
}

struct GamePresence {
    hwnd: Option<isize>,
    misses: u8,
}

impl GamePresence {
    fn new() -> Self {
        Self {
            hwnd: None,
            misses: 0,
        }
    }

    fn observe(&mut self, found: Option<isize>) {
        match found {
            Some(hwnd) => {
                self.hwnd = Some(hwnd);
                self.misses = 0;
            }
            None => {
                self.misses = self.misses.saturating_add(1);
                if self.misses >= 2 {
                    self.hwnd = None;
                }
            }
        }
    }
}

fn should_show(user_visible: bool, game_active: bool, main_in_front: bool) -> bool {
    user_visible && game_active && !main_in_front
}

fn spawn_supervisor(app: AppHandle) {
    std::thread::spawn(move || {
        const TICK_MS: u64 = 250;
        const RECREATE_MS: u64 = 5000;
        let mut previous = snapshot(&app);
        let mut presence = GamePresence::new();
        let mut unfocused_ticks: u8 = 0;
        let mut effective_previous = false;
        let mut last_rect = None;
        let mut since_rect = u64::MAX / 2;
        let mut since_topmost = 0u64;
        let mut since_recreate = u64::MAX / 2;

        loop {
            std::thread::sleep(Duration::from_millis(TICK_MS));
            let current = snapshot(&app);
            let Some(window) = app.get_webview_window("hud") else {
                since_recreate = since_recreate.saturating_add(TICK_MS);
                if since_recreate >= RECREATE_MS {
                    since_recreate = 0;
                    match build_window(&app) {
                        Ok(_) => {
                            effective_previous = false;
                            last_rect = None;
                        }
                        Err(error) => log::warn!("hud recreate failed: {error}"),
                    }
                }
                continue;
            };
            since_recreate = u64::MAX / 2;
            since_rect = since_rect.saturating_add(TICK_MS);
            since_topmost = since_topmost.saturating_add(TICK_MS);

            if since_rect >= current.game_rect_ms {
                since_rect = 0;
                presence.observe(game_window::find_game_window(GAME_PROCESS_NAME));
            }
            let game_present = presence
                .hwnd
                .is_some_and(|hwnd| !game_window::is_iconic(hwnd));
            if game_present && presence.hwnd.is_some_and(game_window::is_foreground) {
                unfocused_ticks = 0;
            } else {
                unfocused_ticks = unfocused_ticks.saturating_add(1);
            }
            let game_active = game_present && unfocused_ticks < 2;
            let effective = should_show(current.visible, game_active, vis::is_foreground("main"));

            if effective != effective_previous {
                if effective {
                    crate::webview_mem::on_shown(&window);
                    if window.show().is_ok() {
                        effective_previous = true;
                        if let Some(hwnd) = vis::hwnd("hud") {
                            overlay::force_topmost(hwnd);
                        }
                        crate::pipeline::resync(&app);
                        last_rect = None;
                    }
                } else if window.hide().is_ok() {
                    effective_previous = false;
                    crate::webview_mem::on_hidden(&window);
                }
            } else if effective && vis::is_visible("hud") == Some(false) {
                crate::webview_mem::on_shown(&window);
                if window.show().is_ok() {
                    if let Some(hwnd) = vis::hwnd("hud") {
                        overlay::force_topmost(hwnd);
                    }
                }
            }

            if current.visible != previous.visible {
                last_rect = None;
            }
            previous = current;
            if !effective_previous {
                continue;
            }

            if let Some(game) = presence.hwnd {
                if let Some(rect) = game_window::client_rect_on_screen(game) {
                    if last_rect != Some(rect) {
                        last_rect = Some(rect);
                        anchor(&window, rect);
                    }
                }
            }
            if since_topmost >= current.topmost_ms {
                since_topmost = 0;
                if let Some(hwnd) = vis::hwnd("hud") {
                    overlay::ensure_topmost(hwnd);
                }
            }
        }
    });
}

fn anchor_position(rect: (i32, i32, i32, i32), scale: f64, width: f64, margin: f64) -> (i32, i32) {
    let (game_x, game_y, game_width, _) = rect;
    let physical_width = (width * scale).round() as i32;
    let physical_margin = (margin * scale).round() as i32;
    (
        game_x + (game_width - physical_width) / 2,
        game_y + physical_margin,
    )
}

fn anchor(window: &tauri::WebviewWindow, rect: (i32, i32, i32, i32)) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let (x, y) = anchor_position(rect, scale, HUD_WIDTH, HUD_TOP_MARGIN);
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_is_centered_inside_the_game_client_area() {
        assert_eq!(
            anchor_position((100, 50, 1_920, 1_080), 1.25, 720.0, 18.0),
            (610, 73),
        );
    }

    #[test]
    fn hud_only_shows_for_an_active_game_and_not_over_the_main_window() {
        assert!(should_show(true, true, false));
        assert!(!should_show(false, true, false));
        assert!(!should_show(true, false, false));
        assert!(!should_show(true, true, true));
    }

    #[test]
    fn hud_window_has_default_capability() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let windows = capability["windows"].as_array().unwrap();

        assert!(
            windows.iter().any(|window| window.as_str() == Some("hud")),
            "the HUD webview must be authorized to invoke commands and emit its ready event"
        );
    }

    #[test]
    fn missing_ready_allows_one_recreation_then_starts_supervisor() {
        let mut recovery = ReadyRecovery::default();
        assert_eq!(recovery.on_timeout(), ReadyAction::Recreate);
        assert_eq!(recovery.on_timeout(), ReadyAction::StartSupervisor);
        assert_eq!(recovery.on_timeout(), ReadyAction::None);
    }
}
