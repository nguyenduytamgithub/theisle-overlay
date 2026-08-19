//! Global hotkeys via RegisterHotKey. Port of `app/hotkeys.py`.
//!
//! Why RegisterHotKey and not SetWindowsHookEx(WH_KEYBOARD_LL): low-level
//! keyboard hooks are exactly what anti-cheat systems watch for (AutoHotkey
//! has been flagged over this). RegisterHotKey only registers with the window
//! manager — no hooks, no touching other processes. Windows posts WM_HOTKEY
//! straight to the registering THREAD's queue.
//!
//! And absolutely never SEND keys to the game — reading the user's presses is
//! normal, injecting keys into the game is cheating.
//!
//! A dedicated thread holds the GetMessageW loop and idles at 0% CPU.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PostThreadMessageW, MSG, WM_HOTKEY, WM_QUIT,
};

use crate::commands::apply_settings_patch;
use crate::settings;
use crate::state::AppState;
use crate::store;

const OPACITY_MIN: f64 = 0.25;
const OPACITY_MAX: f64 = 1.0;
const OPACITY_STEP: f64 = 0.1;
const RADIUS_MIN_M: f64 = 150.0;
const RADIUS_MAX_M: f64 = 3000.0;
const RADIUS_STEP: f64 = 1.35;

/// "Ctrl+Alt+M" -> (modifier flags, virtual key). None when not understood.
/// At least one modifier is REQUIRED so a hotkey cannot steal a bare game key.
pub fn parse_hotkey(spec: &str) -> Option<(u32, u32)> {
    let mut mods: u32 = 0;
    let mut vk: Option<u32> = None;
    for part in spec.split('+') {
        let token = part.trim().to_lowercase();
        if token.is_empty() {
            continue;
        }
        match token.as_str() {
            "ctrl" | "control" => mods |= MOD_CONTROL.0,
            "alt" => mods |= MOD_ALT.0,
            "shift" => mods |= MOD_SHIFT.0,
            "win" | "meta" => mods |= MOD_WIN.0,
            "left" => vk = Some(0x25),
            "up" => vk = Some(0x26),
            "right" => vk = Some(0x27),
            "down" => vk = Some(0x28),
            "space" => vk = Some(0x20),
            "tab" => vk = Some(0x09),
            "enter" | "return" => vk = Some(0x0D),
            "insert" => vk = Some(0x2D),
            "delete" => vk = Some(0x2E),
            "home" => vk = Some(0x24),
            "end" => vk = Some(0x23),
            "pageup" => vk = Some(0x21),
            "pagedown" => vk = Some(0x22),
            "plus" => vk = Some(0xBB),
            "minus" => vk = Some(0xBD),
            t if t.chars().count() == 1 => {
                vk = Some(t.chars().next().unwrap().to_ascii_uppercase() as u32);
            }
            t if t.starts_with('f') && t[1..].chars().all(|c| c.is_ascii_digit()) => {
                if let Ok(n) = t[1..].parse::<u32>() {
                    if (1..=24).contains(&n) {
                        vk = Some(0x70 + n - 1);
                    }
                }
            }
            _ => {}
        }
    }
    // MOD_NOREPEAT is mandatory: without it a held key floods the queue.
    match (vk, mods) {
        (Some(vk), m) if m != 0 => Some((m | MOD_NOREPEAT.0, vk)),
        _ => None,
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedHotkey {
    pub action: String,
    pub spec: String,
}

/// Restartable so the Settings screen can rebind live.
pub struct HotkeyManager {
    thread_id: Mutex<Option<u32>>,
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self {
            thread_id: Mutex::new(None),
        }
    }

    /// (Re)register everything from the current settings. Failures are
    /// aggregated into ONE `hotkey://failed` event (the old app popped a
    /// single QMessageBox for the same reason).
    pub fn restart(&self, app: AppHandle) {
        self.stop();
        let bindings: Vec<(String, String)> = {
            let state = app.state::<AppState>();
            let s = state.settings.lock().unwrap();
            s.get("hotkeys")
                .and_then(|h| h.as_object())
                .map(|h| {
                    h.iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                        .collect()
                })
                .unwrap_or_default()
        };

        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<u32>();
        let thread_app = app.clone();
        std::thread::spawn(move || {
            let thread_id = unsafe { GetCurrentThreadId() };
            let _ = ready_tx.send(thread_id);

            let mut registered: Vec<(i32, String)> = Vec::new();
            let mut failed: Vec<FailedHotkey> = Vec::new();
            for (index, (action, spec)) in bindings.iter().enumerate() {
                let id = index as i32 + 1;
                match parse_hotkey(spec) {
                    Some((mods, vk)) => unsafe {
                        if RegisterHotKey(None, id, HOT_KEY_MODIFIERS(mods), vk).is_ok() {
                            registered.push((id, action.clone()));
                        } else {
                            // Usually another app holds this combination.
                            failed.push(FailedHotkey {
                                action: action.clone(),
                                spec: spec.clone(),
                            });
                        }
                    },
                    None => failed.push(FailedHotkey {
                        action: action.clone(),
                        spec: spec.clone(),
                    }),
                }
            }
            if !failed.is_empty() {
                let _ = thread_app.emit("hotkey://failed", failed);
            }

            let mut msg = MSG::default();
            unsafe {
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    if msg.message == WM_HOTKEY {
                        let id = msg.wParam.0 as i32;
                        if let Some((_, action)) =
                            registered.iter().find(|(rid, _)| *rid == id)
                        {
                            dispatch(&thread_app, action);
                        }
                    }
                }
                for (id, _) in &registered {
                    let _ = UnregisterHotKey(None, *id);
                }
            }
        });

        if let Ok(thread_id) = ready_rx.recv_timeout(std::time::Duration::from_secs(2)) {
            *self.thread_id.lock().unwrap() = Some(thread_id);
        }
    }

    pub fn stop(&self) {
        if let Some(thread_id) = self.thread_id.lock().unwrap().take() {
            unsafe {
                let _ = PostThreadMessageW(
                    thread_id,
                    WM_QUIT,
                    windows::Win32::Foundation::WPARAM(0),
                    windows::Win32::Foundation::LPARAM(0),
                );
            }
        }
    }
}

/// A hotkey action. Everything routes through the settings patch path so
/// every window (and the debounced save) reacts the same way regardless of
/// whether the change came from a hotkey or the Settings screen.
fn dispatch(app: &AppHandle, action: &str) {
    log::info!("hotkey: {action}");
    match action {
        "toggle_minimap" => toggle_setting(app, "visible"),
        "toggle_click_through" => toggle_setting(app, "click_through"),
        "toggle_fullmap" => match app.get_webview_window("main") {
            Some(window) => {
                // A minimized window reports is_visible() == true, but the
                // user cannot see it — for them the hotkey means "bring it
                // up", so only a window actually on screen gets hidden.
                let on_screen = window.is_visible().unwrap_or(false)
                    && !window.is_minimized().unwrap_or(false);
                if on_screen {
                    let _ = window.hide();
                    // Freeze the hidden webview so its renderer memory is
                    // actually released while playing.
                    crate::webview_mem::suspend(&window);
                } else {
                    crate::webview_mem::resume(&window);
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                    // It missed every event while hidden/suspended.
                    crate::pipeline::resync(app);
                }
            }
            None => {
                // The user closed it with the X button — recreate it from
                // the same config it was born with.
                if let Some(config) = app.config().app.windows.first().cloned() {
                    match tauri::WebviewWindowBuilder::from_config(app, &config) {
                        Ok(builder) => match builder.build() {
                            Ok(window) => {
                                let _ = window.set_focus();
                            }
                            Err(e) => log::warn!("recreating main window failed: {e}"),
                        },
                        Err(e) => log::warn!("main window config invalid: {e}"),
                    }
                }
            }
        },
        "mark_here" => mark_here(app),
        "opacity_up" => adjust_opacity(app, OPACITY_STEP),
        "opacity_down" => adjust_opacity(app, -OPACITY_STEP),
        "zoom_in" => adjust_radius(app, 1.0 / RADIUS_STEP),
        "zoom_out" => adjust_radius(app, RADIUS_STEP),
        _ => {}
    }
}

fn toggle_setting(app: &AppHandle, key: &str) {
    let current = {
        let state = app.state::<AppState>();
        let s = state.settings.lock().unwrap();
        settings::get_bool(&s, &["minimap", key], true)
    };
    apply_settings_patch(app, serde_json::json!({ "minimap": { key: !current } }));
}

fn adjust_opacity(app: &AppHandle, delta: f64) {
    let current = {
        let state = app.state::<AppState>();
        let s = state.settings.lock().unwrap();
        settings::get_f64(&s, &["minimap", "opacity"], 0.85)
    };
    let next = ((current + delta).clamp(OPACITY_MIN, OPACITY_MAX) * 100.0).round() / 100.0;
    apply_settings_patch(app, serde_json::json!({ "minimap": { "opacity": next } }));
}

fn adjust_radius(app: &AppHandle, factor: f64) {
    let current = {
        let state = app.state::<AppState>();
        let s = state.settings.lock().unwrap();
        settings::get_f64(&s, &["minimap", "radius_m"], 600.0)
    };
    let next = (current * factor).clamp(RADIUS_MIN_M, RADIUS_MAX_M).round();
    apply_settings_patch(app, serde_json::json!({ "minimap": { "radius_m": next } }));
}

/// Drop a waypoint at the current position. The waypoint NAME is data (it is
/// stored in the user's file), so it is localised at creation time.
fn mark_here(app: &AppHandle) {
    let state = app.state::<AppState>();
    let current = {
        let tracker = state.tracker.lock().unwrap();
        tracker.current
    };
    let Some(current) = current else { return };
    let name = {
        let s = state.settings.lock().unwrap();
        match settings::get_str(&s, &["language"], "vi") {
            "en" => "My position",
            _ => "Vị trí của tôi",
        }
    };
    let wp = store::new_waypoint(name, current.x, current.y, current.z, None);
    {
        let mut waypoints = state.waypoints.lock().unwrap();
        waypoints.push(wp);
        if let Err(e) = store::save_waypoints(&waypoints) {
            log::warn!("saving waypoints failed: {e}");
        }
    }
    crate::events::emit_to_visible(app, "waypoints://changed", ());
}

#[cfg(test)]
mod tests {
    use super::parse_hotkey;

    #[test]
    fn parses_the_default_table() {
        // (mods | MOD_NOREPEAT, vk)
        assert_eq!(parse_hotkey("Ctrl+Alt+M"), Some((0x2 | 0x1 | 0x4000, b'M' as u32)));
        assert_eq!(parse_hotkey("Ctrl+Alt+Up"), Some((0x2 | 0x1 | 0x4000, 0x26)));
        assert_eq!(parse_hotkey("Ctrl+Shift+F5"), Some((0x2 | 0x4 | 0x4000, 0x74)));
        assert_eq!(parse_hotkey("Win+Plus"), Some((0x8 | 0x4000, 0xBB)));
    }

    #[test]
    fn requires_a_modifier() {
        assert_eq!(parse_hotkey("M"), None, "bare keys would steal game input");
        assert_eq!(parse_hotkey("F5"), None);
    }

    #[test]
    fn rejects_nonsense() {
        assert_eq!(parse_hotkey(""), None);
        assert_eq!(parse_hotkey("Ctrl+"), None);
        assert_eq!(parse_hotkey("Ctrl+NotAKey"), None);
        assert_eq!(parse_hotkey("F99+Ctrl"), None);
    }
}
