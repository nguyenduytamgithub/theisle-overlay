//! Label -> HWND registry so background threads can answer "is this window
//! visible?" without the tauri getters — those block on a channel round-trip
//! into the main event loop with no timeout, so a stalled main loop would
//! freeze every thread that asks (the hotkey pump most fatally).
//! IsWindowVisible / IsIconic are plain user32 reads on our own cached HWNDs.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindowVisible};

use crate::state::LockExt;

static WINDOWS: LazyLock<Mutex<HashMap<String, isize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn register(label: &str, hwnd: isize) {
    WINDOWS.lock_safe().insert(label.to_string(), hwnd);
}

pub fn unregister(label: &str) {
    WINDOWS.lock_safe().remove(label);
}

pub fn hwnd(label: &str) -> Option<isize> {
    WINDOWS.lock_safe().get(label).copied()
}

/// None when the label was never registered (e.g. the login window) — callers
/// fall back to the blocking getter for those. A destroyed HWND reports
/// not-visible, which is the right answer for every caller.
pub fn is_visible(label: &str) -> Option<bool> {
    let raw = hwnd(label)?;
    Some(unsafe { IsWindowVisible(HWND(raw as *mut std::ffi::c_void)).as_bool() })
}

pub fn is_minimized(label: &str) -> Option<bool> {
    let raw = hwnd(label)?;
    Some(unsafe { IsIconic(HWND(raw as *mut std::ffi::c_void)).as_bool() })
}

/// Wait briefly until the window reports visible. `show()` is posted to the
/// main event loop, so an emit gated on IsWindowVisible right after it could
/// otherwise race the show and skip the window it was meant for.
pub fn wait_visible(label: &str, timeout_ms: u64) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        if is_visible(label) == Some(true) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
