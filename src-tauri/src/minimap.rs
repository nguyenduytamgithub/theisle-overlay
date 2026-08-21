//! The minimap overlay window: creation, game-window anchoring, topmost
//! re-assertion, click-through. Port of the window-management half of the
//! original `main.py` + `minimap.py`.
//!
//! The window is created hidden; the supervisor is wired up when the webview
//! signals `minimap://ready` (kills the WebView2 white-flash-on-startup, with
//! a timeout fallback so a broken webview can't leave the overlay dead).
//!
//! One supervisor thread replaces Qt's three timers. It ticks at 250 ms and
//! owns the ONLY show/hide path: what the user sees is
//! `user_visible && (!require_game || game running)` — the `visible` setting
//! stays pure user intent, and game presence (polled every game_rect_ms even
//! while hidden) gates it. Anchoring and topmost run only while shown. There
//! are still no repaint timers anywhere — the webview draws only on events.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tauri::{
    AppHandle, Listener, LogicalSize, Manager, PhysicalPosition, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::settings::{self, GAME_PROCESS_NAME};
use crate::state::{AppState, LockExt};
use crate::win::{game_window, overlay, vis};

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    // Include the dino strip in the initial size, not just on later changes.
    let snap = snapshot(app);
    let (size, height) = (snap.size_px, snap.window_h());

    let window = WebviewWindowBuilder::new(app, "minimap", WebviewUrl::App("minimap.html".into()))
        .title("minimap")
        .inner_size(size, height)
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

    // Belt-and-braces: assert NOACTIVATE + TOOLWINDOW on the raw HWND no
    // matter what the windowing library set.
    if let Ok(hwnd) = window.hwnd() {
        let raw = hwnd.0 as isize;
        vis::register("minimap", raw);
        overlay::assert_overlay_styles(raw);
    }

    let app_handle = app.clone();
    let shown = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ready_guard = shown.clone();
    app.listen_any("minimap://ready", move |_| {
        // The webview can reload during dev; only wire things up once.
        if ready_guard.swap(true, Ordering::SeqCst) {
            return;
        }
        on_ready(&app_handle);
    });

    // Fallback: if the webview never signals ready (a script error before
    // its emit), wire up anyway — an overlay that no hotkey can ever revive
    // is the worst failure mode this window has.
    let fallback_app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        if !shown.swap(true, Ordering::SeqCst) {
            log::warn!("minimap://ready never arrived; starting supervisor anyway");
            on_ready(&fallback_app);
        }
    });

    Ok(())
}

fn on_ready(app: &AppHandle) {
    let state = app.state::<AppState>();
    let click_through = {
        let s = state.settings.lock_safe();
        settings::get_bool(&s, &["minimap", "click_through"], true)
    };
    if let Some(window) = app.get_webview_window("minimap") {
        let _ = window.set_ignore_cursor_events(click_through);
    }
    // Showing is the supervisor's job — one show path, resync included.
    spawn_supervisor(app.clone());
}

/// Snapshot of the minimap-relevant settings, compared tick-to-tick so work
/// only happens on change.
/// Height of the dino-stats strip under the map disc, logical px. Must match
/// PANEL_H in src/minimap/render.ts.
const DINO_PANEL_H: f64 = 76.0;

#[derive(PartialEq, Clone, Copy)]
struct Snapshot {
    /// The user's intent (hotkey / Settings toggle) — game presence gates it
    /// but never writes it.
    user_visible: bool,
    require_game: bool,
    click_through: bool,
    size_px: f64,
    margin_px: f64,
    corner: Corner,
    game_rect_ms: u64,
    topmost_ms: u64,
    /// Extra height for the "your dino" stats panel.
    panel_h: f64,
}

impl Snapshot {
    fn window_h(&self) -> f64 {
        self.size_px + self.panel_h
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    fn parse(s: &str) -> Self {
        match s {
            "top-right" => Self::TopRight,
            "bottom-left" => Self::BottomLeft,
            "bottom-right" => Self::BottomRight,
            _ => Self::TopLeft,
        }
    }
}

fn snapshot(app: &AppHandle) -> Snapshot {
    let state = app.state::<AppState>();
    let s = state.settings.lock_safe();
    Snapshot {
        user_visible: settings::get_bool(&s, &["minimap", "visible"], true),
        require_game: settings::get_bool(&s, &["minimap", "require_game"], true),
        click_through: settings::get_bool(&s, &["minimap", "click_through"], true),
        size_px: settings::get_f64(&s, &["minimap", "size_px"], 260.0),
        margin_px: settings::get_f64(&s, &["minimap", "margin_px"], 16.0),
        corner: Corner::parse(settings::get_str(&s, &["minimap", "corner"], "top-left")),
        game_rect_ms: settings::get_f64(&s, &["poll", "game_rect_ms"], 1000.0) as u64,
        topmost_ms: settings::get_f64(&s, &["poll", "topmost_ms"], 2000.0) as u64,
        panel_h: if settings::get_bool(&s, &["islepilot", "enabled"], false)
            && settings::get_bool(&s, &["islepilot", "show_overlay_panel"], true)
        {
            DINO_PANEL_H
        } else {
            0.0
        },
    }
}

/// Debounced game-window presence: appears on the first sighting, drops only
/// after MISS_LIMIT consecutive misses — a poll hiccup (or a brief window
/// recreation inside the game) must not flicker the overlay.
pub(crate) struct GamePresence {
    hwnd: Option<isize>,
    misses: u8,
}

const MISS_LIMIT: u8 = 2;

impl GamePresence {
    pub(crate) fn new() -> Self {
        Self {
            hwnd: None,
            misses: 0,
        }
    }

    pub(crate) fn observe(&mut self, found: Option<isize>) -> Option<isize> {
        match found {
            Some(h) => {
                self.hwnd = Some(h);
                self.misses = 0;
            }
            None => {
                self.misses = self.misses.saturating_add(1);
                if self.misses >= MISS_LIMIT {
                    self.hwnd = None;
                }
            }
        }
        self.hwnd
    }

    pub(crate) fn hwnd(&self) -> Option<isize> {
        self.hwnd
    }
}

fn spawn_supervisor(app: AppHandle) {
    std::thread::spawn(move || {
        const TICK_MS: u64 = 250;
        let mut prev = snapshot(&app);
        let mut presence = GamePresence::new();
        // The window was created hidden; the first tick decides the show.
        let mut effective_prev = false;
        let mut last_rect: Option<(i32, i32, i32, i32)> = None;
        let mut since_rect: u64 = u64::MAX / 2; // fire immediately
        let mut since_topmost: u64 = 0;

        loop {
            std::thread::sleep(Duration::from_millis(TICK_MS));
            let cur = snapshot(&app);
            let Some(window) = app.get_webview_window("minimap") else {
                continue;
            };

            since_rect += TICK_MS;
            since_topmost += TICK_MS;

            // Presence is polled even while hidden — it is what un-hides us.
            if since_rect >= cur.game_rect_ms {
                since_rect = 0;
                presence.observe(game_window::find_game_window(GAME_PROCESS_NAME));
            }
            // IsIconic every tick (cheap): a minimized game must drop the
            // overlay within one tick, not one poll interval.
            let game_present = presence.hwnd().is_some_and(|h| !game_window::is_iconic(h));

            let effective = cur.user_visible && (!cur.require_game || game_present);
            if effective != effective_prev {
                if effective {
                    log::info!("minimap: show (game_present={game_present})");
                    crate::webview_mem::resume(&window);
                    if window.show().is_ok() {
                        effective_prev = true;
                        // No 2 s topmost gap on an auto-show...
                        if let Some(h) = vis::hwnd("minimap") {
                            overlay::ensure_topmost(h);
                        }
                        // ...and catch up on everything missed while hidden.
                        // show() is posted to the event loop; wait for it to
                        // land or the resync's visibility gate skips us.
                        vis::wait_visible("minimap", 200);
                        crate::pipeline::resync(&app);
                        last_rect = None; // re-anchor right away
                    }
                } else if window.hide().is_ok() {
                    log::info!("minimap: hide (user={}, game={game_present})", cur.user_visible);
                    effective_prev = false;
                    crate::webview_mem::suspend(&window);
                }
                // A failed show/hide leaves effective_prev unchanged, so the
                // transition is retried next tick instead of swallowed.
            } else if effective && vis::is_visible("minimap") == Some(false) {
                // The OS hid us (or a show was lost) while we believe we are
                // on screen — re-apply idempotently, no resync spam.
                crate::webview_mem::resume(&window);
                let _ = window.show();
            }

            if cur.click_through != prev.click_through {
                let _ = window.set_ignore_cursor_events(cur.click_through);
            }
            if cur.size_px != prev.size_px || cur.panel_h != prev.panel_h {
                let _ = window.set_size(LogicalSize::new(cur.size_px, cur.window_h()));
                last_rect = None;
            }
            if cur.corner != prev.corner || cur.margin_px != prev.margin_px {
                last_rect = None;
            }
            prev = cur;

            if !effective_prev {
                continue;
            }

            // Anchor to the game's client area every tick (4 cheap reads/s);
            // the rect comparison keeps repositioning to actual moves.
            if let Some(game) = presence.hwnd() {
                if let Some(rect) = game_window::client_rect_on_screen(game) {
                    if last_rect != Some(rect) {
                        last_rect = Some(rect);
                        anchor(&window, rect, &cur);
                    }
                }
            }

            if since_topmost >= cur.topmost_ms {
                since_topmost = 0;
                if let Some(hwnd) = vis::hwnd("minimap") {
                    // Checks the style bit first — no needless DWM repaints.
                    overlay::ensure_topmost(hwnd);
                }
            }
        }
    });
}

/// Pin the overlay to a corner of the game's client area. All arithmetic in
/// PHYSICAL pixels (Win32 gives physical, and margins/sizes are logical, so
/// they scale by the window's DPI factor — the machine runs at 125%).
fn anchor(window: &tauri::WebviewWindow, rect: (i32, i32, i32, i32), snap: &Snapshot) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = (snap.size_px * scale).round() as i32;
    let height = (snap.window_h() * scale).round() as i32;
    let margin = (snap.margin_px * scale).round() as i32;
    let (gx, gy, gw, gh) = rect;

    let x = match snap.corner {
        Corner::TopLeft | Corner::BottomLeft => gx + margin,
        Corner::TopRight | Corner::BottomRight => gx + gw - size - margin,
    };
    let y = match snap.corner {
        Corner::TopLeft | Corner::TopRight => gy + margin,
        Corner::BottomLeft | Corner::BottomRight => gy + gh - height - margin,
    };
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

#[cfg(test)]
mod tests {
    use super::GamePresence;

    #[test]
    fn presence_appears_immediately_and_survives_one_miss() {
        let mut p = GamePresence::new();
        assert_eq!(p.observe(None), None);
        assert_eq!(p.observe(Some(7)), Some(7), "first sighting shows at once");
        assert_eq!(p.observe(None), Some(7), "one miss is a poll hiccup");
        assert_eq!(p.observe(Some(7)), Some(7), "recovery resets the misses");
        assert_eq!(p.observe(None), Some(7));
        assert_eq!(p.observe(None), None, "two consecutive misses = game gone");
        assert_eq!(p.observe(Some(9)), Some(9), "reappearance is immediate");
    }
}
