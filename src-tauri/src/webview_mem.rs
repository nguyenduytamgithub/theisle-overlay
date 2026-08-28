//! WebView2 memory management for hidden windows.
//!
//! Hiding a window does not release its renderer memory. The app used to
//! answer that with `TrySuspend` — but TrySuspend is ASYNCHRONOUS inside
//! WebView2, and the suspend/resume races kept producing a visible window
//! whose input was dead (three distinct field incidents, each needing its
//! own mitigation: settle delays, generation tokens, a two-way watchdog).
//! ~80 MB of idle savings is not worth an overlay that stops taking clicks
//! mid-game, so suspension is gone BY DESIGN: hidden windows only get
//! `MemoryUsageTargetLevel::Low` — a synchronous cache-trim hint with no
//! execution semantics — restored to Normal on show. Because hidden webviews
//! stay alive, events are simply broadcast to them (events::emit_all) and a
//! re-shown window is already current.
//!
//! The watchdog stays as a sentinel: nothing should ever be suspended or
//! controller-hidden again, and it logs loudly if that assumption breaks.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use tauri::WebviewWindow;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2_19, ICoreWebView2_3, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
};
// NOT windows::core — webview2-com's interfaces are generated against
// windows 0.61, so `cast` comes from that version's Interface trait.
use windows_core::Interface;

use crate::state::LockExt;

/// Don't bother trimming for a quick hide/show toggle.
const SETTLE_MS: u64 = 1500;

/// Per-window-label cancellation token: bumped by on_shown() and by every
/// new on_hidden() request, so only the latest pending trim can fire.
static GEN: LazyLock<Mutex<HashMap<String, u64>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn bump_gen(label: &str) -> u64 {
    let mut map = GEN.lock_safe();
    let entry = map.entry(label.to_string()).or_insert(0);
    *entry += 1;
    *entry
}

fn current_gen(label: &str) -> u64 {
    *GEN.lock_safe().get(label).unwrap_or(&0)
}

/// The window was hidden: once that has settled, trim its memory target.
/// Cancelled automatically if it is shown again in the meantime. Fail-soft.
pub fn on_hidden(window: &WebviewWindow) {
    let label = window.label().to_string();
    let generation = bump_gen(&label);
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(SETTLE_MS));
        if current_gen(&label) != generation {
            return; // shown again meanwhile — stand down
        }
        // Registry read, not the tauri getter: this sleep-thread must never
        // block on the main event loop. Unknown label -> treat as visible.
        if crate::win::vis::is_visible(&label).unwrap_or(true) {
            return;
        }
        reduce_memory(&window);
    });
}

fn reduce_memory(window: &WebviewWindow) {
    let result = window.with_webview(|webview| unsafe {
        let controller = webview.controller();
        let Ok(core) = controller.CoreWebView2() else {
            return;
        };
        if let Ok(wv19) = core.cast::<ICoreWebView2_19>() {
            let _ = wv19.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW);
        }
    });
    if let Err(e) = result {
        log::warn!("memory target failed: {e}");
    }
}

/// The window is being shown: cancel any pending trim, restore the memory
/// target, and make sure the controller is visible and taking input.
pub fn on_shown(window: &WebviewWindow) {
    bump_gen(window.label());
    let result = window.with_webview(|webview| unsafe {
        let controller = webview.controller();
        let Ok(core) = controller.CoreWebView2() else {
            let _ = controller.SetIsVisible(true);
            return;
        };
        // Nothing suspends webviews any more; Resume stays as belt-and-braces.
        if let Ok(wv3) = core.cast::<ICoreWebView2_3>() {
            let _ = wv3.Resume();
        }
        if let Ok(wv19) = core.cast::<ICoreWebView2_19>() {
            let _ = wv19.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL);
        }
        let _ = controller.SetIsVisible(true);
        // After visibility cycling WebView2 can leave the render widget
        // painting fine but accepting no clicks; this nudge re-syncs its
        // input routing (the documented cure for stale-input states).
        let _ = controller.NotifyParentWindowPositionChanged();
    });
    if let Err(e) = result {
        log::warn!("webview restore failed: {e}");
    }
}

/// Sentinel: no webview should ever be suspended or controller-hidden while
/// its window is on screen. If one is found anyway, heal it and log loudly —
/// that log line means an assumption above has broken.
pub fn spawn_watchdog(app: tauri::AppHandle) {
    use tauri::Manager;
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(500));
        for label in ["main", "minimap", "night-vision"] {
            if crate::win::vis::is_visible(label) != Some(true) {
                continue;
            }
            let Some(window) = app.get_webview_window(label) else {
                continue;
            };
            let owned = label.to_string();
            let _ = window.with_webview(move |webview| unsafe {
                let controller = webview.controller();
                let mut ctrl_visible = windows_core::BOOL::default();
                if controller.IsVisible(&mut ctrl_visible).is_ok() && !ctrl_visible.as_bool() {
                    log::warn!("webview '{owned}' visible but controller hidden — self-healing");
                    let _ = controller.SetIsVisible(true);
                    let _ = controller.NotifyParentWindowPositionChanged();
                }
                let Ok(core) = controller.CoreWebView2() else {
                    return;
                };
                let Ok(wv3) = core.cast::<ICoreWebView2_3>() else {
                    return;
                };
                let mut suspended = windows_core::BOOL::default();
                if wv3.IsSuspended(&mut suspended).is_err() || !suspended.as_bool() {
                    return;
                }
                log::warn!("webview '{owned}' visible but suspended — self-healing");
                if let Ok(wv19) = core.cast::<ICoreWebView2_19>() {
                    let _ = wv19
                        .SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL);
                }
                let _ = wv3.Resume();
                let _ = controller.SetIsVisible(true);
                let _ = controller.NotifyParentWindowPositionChanged();
            });
        }
    });
}
