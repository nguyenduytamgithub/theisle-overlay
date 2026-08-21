//! System tray: the app lives here while the main window is hidden, the way
//! Steam/Discord do it. Closing the main window with X hides it (see the
//! CloseRequested handler in lib.rs); the tray menu is the always-available
//! way back in — and the only ordinary way to actually quit.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::menu::MenuBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::state::{AppState, LockExt};
use crate::win::vis;

/// Set before app.exit so an in-flight CloseRequested cannot re-hide the
/// window mid-quit. app.exit itself bypasses CloseRequested; this flag is
/// belt-and-braces.
static QUITTING: AtomicBool = AtomicBool::new(false);

pub fn is_quitting() -> bool {
    QUITTING.load(Ordering::SeqCst)
}

/// Menu labels are Rust-side data, so they are localised here (same precedent
/// as the `mark_here` waypoint name in hotkeys.rs).
fn labels(app: &AppHandle) -> (&'static str, &'static str) {
    let state = app.state::<AppState>();
    let lang = {
        let s = state.settings.lock_safe();
        crate::settings::get_str(&s, &["language"], "vi").to_string()
    };
    match lang.as_str() {
        "en" => ("Show window", "Quit"),
        _ => ("Hiện cửa sổ", "Thoát"),
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let (show, quit) = labels(app);
    let menu = MenuBuilder::new(app)
        .text("show", show)
        .separator()
        .text("quit", quit)
        .build()?;
    let icon = app
        .default_window_icon()
        .expect("bundled window icon")
        .clone();
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("TheIsle Overlay")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "quit" => {
                QUITTING.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// The ONE way to bring the main window up, from any thread: resume the
/// webview first (it is suspended while hidden), then show/focus, then
/// resync the events it missed — a bare show() would present a stale page.
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        crate::webview_mem::resume(&window);
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        // show() is posted to the event loop; wait for it to land or the
        // resync's visibility gate skips the very window it is for.
        vis::wait_visible("main", 200);
        crate::pipeline::resync(app);
    }
}

/// Language changed: swap the menu for one in the new language. Tray menus
/// are main-thread-affine on Windows, and settings patches can arrive from
/// the hotkey worker — hop explicitly.
pub fn rebuild_menu(app: &AppHandle) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        let (show, quit) = labels(&app);
        let menu = MenuBuilder::new(&app)
            .text("show", show)
            .separator()
            .text("quit", quit)
            .build();
        if let (Some(tray), Ok(menu)) = (app.tray_by_id("main"), menu) {
            let _ = tray.set_menu(Some(menu));
        }
    });
}
