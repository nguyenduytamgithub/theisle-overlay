//! The minimap overlay window: creation, game-window anchoring, topmost
//! re-assertion, click-through. Port of the window-management half of the
//! original `main.py` + `minimap.py`.
//!
//! The window is created hidden and only shown after the webview signals
//! `minimap://ready` — this kills the WebView2 white-flash-on-startup.
//!
//! One supervisor thread replaces Qt's three timers. It ticks at 250 ms and
//! fires each job on its own cadence (game rect 1 s, topmost 2 s, both only
//! while the minimap is visible). There are still no repaint timers anywhere —
//! the webview draws only on events.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{
    AppHandle, Listener, LogicalSize, Manager, PhysicalPosition, WebviewUrl,
    WebviewWindowBuilder,
};

use crate::settings::{self, GAME_PROCESS_NAME};
use crate::state::AppState;
use crate::win::{game_window, overlay};

static MINIMAP_HWND: AtomicIsize = AtomicIsize::new(0);

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
        MINIMAP_HWND.store(raw, Ordering::Relaxed);
        overlay::assert_overlay_styles(raw);
    }

    let app_handle = app.clone();
    let shown = Arc::new(std::sync::atomic::AtomicBool::new(false));
    app.listen_any("minimap://ready", move |_| {
        // The webview can reload during dev; only wire things up once.
        if shown.swap(true, Ordering::SeqCst) {
            return;
        }
        on_ready(&app_handle);
    });

    Ok(())
}

fn on_ready(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (visible, click_through) = {
        let s = state.settings.lock().unwrap();
        (
            settings::get_bool(&s, &["minimap", "visible"], true),
            settings::get_bool(&s, &["minimap", "click_through"], true),
        )
    };
    if let Some(window) = app.get_webview_window("minimap") {
        let _ = window.set_ignore_cursor_events(click_through);
        if visible {
            let _ = window.show();
        }
    }
    spawn_supervisor(app.clone());
}

/// Snapshot of the minimap-relevant settings, compared tick-to-tick so work
/// only happens on change.
/// Height of the dino-stats strip under the map disc, logical px. Must match
/// PANEL_H in src/minimap/render.ts.
const DINO_PANEL_H: f64 = 76.0;

#[derive(PartialEq, Clone, Copy)]
struct Snapshot {
    visible: bool,
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
    let s = state.settings.lock().unwrap();
    Snapshot {
        visible: settings::get_bool(&s, &["minimap", "visible"], true),
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

fn spawn_supervisor(app: AppHandle) {
    std::thread::spawn(move || {
        const TICK_MS: u64 = 250;
        let mut prev = snapshot(&app);
        let mut last_rect: Option<(i32, i32, i32, i32)> = None;
        let mut since_rect: u64 = u64::MAX / 2; // fire immediately
        let mut since_topmost: u64 = 0;

        loop {
            std::thread::sleep(Duration::from_millis(TICK_MS));
            let cur = snapshot(&app);
            let Some(window) = app.get_webview_window("minimap") else {
                continue;
            };

            if cur.visible != prev.visible {
                if cur.visible {
                    crate::webview_mem::resume(&window);
                    let _ = window.show();
                    // Catch up on everything it missed while suspended.
                    crate::pipeline::resync(&app);
                    last_rect = None; // re-anchor right away
                    since_rect = u64::MAX / 2;
                } else {
                    let _ = window.hide();
                    crate::webview_mem::suspend(&window);
                }
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

            if !cur.visible {
                continue;
            }

            since_rect += TICK_MS;
            since_topmost += TICK_MS;

            if since_rect >= cur.game_rect_ms {
                since_rect = 0;
                if let Some(game) = game_window::find_game_window(GAME_PROCESS_NAME) {
                    if let Some(rect) = game_window::client_rect_on_screen(game) {
                        // Only reposition when the game window actually moved.
                        if last_rect != Some(rect) {
                            last_rect = Some(rect);
                            anchor(&window, rect, &cur);
                        }
                    }
                }
            }

            if since_topmost >= cur.topmost_ms {
                since_topmost = 0;
                let hwnd = MINIMAP_HWND.load(Ordering::Relaxed);
                if hwnd != 0 {
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
