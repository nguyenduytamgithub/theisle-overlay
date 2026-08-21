//! WebView2 memory management for hidden windows.
//!
//! Hiding a window does NOT release its renderer memory (measured: 525 MB
//! before and after hiding the full map). WebView2 ships a purpose-built
//! answer: `TrySuspend` freezes a hidden webview and releases most of its
//! renderer memory, and `MemoryUsageTargetLevel::Low` tells the browser to
//! trim caches aggressively. Resume is near-instant.
//!
//! CRITICAL COMPANION RULE: calling almost any WebView2 API — including the
//! script evaluation behind every tauri `emit` — auto-resumes a suspended
//! webview. That is why the app routes UI events through
//! `events::emit_to_visible` (skip hidden windows) and re-syncs state via
//! `pipeline::resync` when a window is shown again. Broadcasting to a
//! suspended window would silently wake it and undo the savings.

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

/// TrySuspend is ASYNC: rapid hide→show toggling can land the suspension
/// AFTER the resume, leaving a VISIBLE but frozen webview — the window shows
/// its last frame while every click is dead (observed in the field with fast
/// Ctrl+Alt+F spam). The cure is to never suspend immediately: wait for the
/// window to stay hidden for this long first, and cancel on any resume.
const SETTLE_MS: u64 = 1500;

/// Per-window-label cancellation token: bumped by resume() and by every new
/// suspend request, so only the latest pending suspension can fire.
static SUSPEND_GEN: LazyLock<Mutex<HashMap<String, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn bump_gen(label: &str) -> u64 {
    let mut map = SUSPEND_GEN.lock_safe();
    let entry = map.entry(label.to_string()).or_insert(0);
    *entry += 1;
    *entry
}

fn current_gen(label: &str) -> u64 {
    *SUSPEND_GEN.lock_safe().get(label).unwrap_or(&0)
}

/// Freeze a hidden window's webview once it has stayed hidden for SETTLE_MS.
/// Cancelled automatically if `resume` (or another suspend request) happens
/// in the meantime. Fail-soft: on any error the window keeps its memory.
pub fn suspend(window: &WebviewWindow) {
    let label = window.label().to_string();
    let generation = bump_gen(&label);
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(SETTLE_MS));
        if current_gen(&label) != generation {
            return; // shown again (or re-hidden) meanwhile — stand down
        }
        // Registry read, not the tauri getter: this sleep-thread must never
        // block on the main event loop. Unknown label -> treat as visible.
        if crate::win::vis::is_visible(&label).unwrap_or(true) {
            return; // safety: never suspend something the user can see
        }
        suspend_now(&window);
    });
}

fn suspend_now(window: &WebviewWindow) {
    let label = window.label().to_string();
    let result = window.with_webview(move |webview| unsafe {
        let controller = webview.controller();
        // TrySuspend requires the webview to be invisible.
        let _ = controller.SetIsVisible(false);
        let Ok(core) = controller.CoreWebView2() else {
            return;
        };
        if let Ok(wv19) = core.cast::<ICoreWebView2_19>() {
            let _ = wv19.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW);
        }
        if let Ok(wv3) = core.cast::<ICoreWebView2_3>() {
            let inner_label = label.clone();
            let handler = webview2_com::TrySuspendCompletedHandler::create(Box::new(
                move |result, is_suspended| {
                    log::info!(
                        "webview '{inner_label}' suspend: result={result:?} suspended={is_suspended:?}"
                    );
                    Ok(())
                },
            ));
            let _ = wv3.TrySuspend(&handler);
        }
    });
    if let Err(e) = result {
        log::warn!("suspend webview failed: {e}");
    }
}

/// Undo `suspend`. Called before showing the window again. Cancels any
/// pending suspension, resumes immediately, and resumes ONCE MORE shortly
/// after — catching a suspension that was already in flight inside WebView2
/// when the user brought the window back.
pub fn resume(window: &WebviewWindow) {
    bump_gen(window.label());
    resume_now(window);
    let label = window.label().to_string();
    let generation = current_gen(&label);
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(600));
        if current_gen(&label) == generation
            && crate::win::vis::is_visible(&label).unwrap_or(false)
        {
            resume_now(&window);
        }
    });
}

/// Last line of defence against the residual TrySuspend race: a suspension
/// that lands AFTER the final resume leaves a visible window frozen (last
/// frame painted, every click dead) with nothing to wake it. Poll the real
/// suspension state of VISIBLE windows and undo it. Hidden windows are never
/// touched, so the memory savings stay intact. Reading IsSuspended is the
/// documented state check and does not itself resume — and even if it did,
/// resuming a visible window is exactly the intent.
pub fn spawn_watchdog(app: tauri::AppHandle) {
    use tauri::Manager;
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(500));
        for label in ["main", "minimap"] {
            if crate::win::vis::is_visible(label) != Some(true) {
                continue;
            }
            let Some(window) = app.get_webview_window(label) else {
                continue;
            };
            let owned = label.to_string();
            let _ = window.with_webview(move |webview| unsafe {
                let controller = webview.controller();
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
                bump_gen(&owned); // cancel any pending suspend for this label
                if let Ok(wv19) = core.cast::<ICoreWebView2_19>() {
                    let _ = wv19
                        .SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL);
                }
                let _ = wv3.Resume();
                let _ = controller.SetIsVisible(true);
            });
        }
    });
}

fn resume_now(window: &WebviewWindow) {
    let result = window.with_webview(|webview| unsafe {
        let controller = webview.controller();
        let Ok(core) = controller.CoreWebView2() else {
            let _ = controller.SetIsVisible(true);
            return;
        };
        if let Ok(wv3) = core.cast::<ICoreWebView2_3>() {
            let _ = wv3.Resume();
        }
        if let Ok(wv19) = core.cast::<ICoreWebView2_19>() {
            let _ = wv19.SetMemoryUsageTargetLevel(COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL);
        }
        let _ = controller.SetIsVisible(true);
    });
    if let Err(e) = result {
        log::warn!("resume webview failed: {e}");
    }
}
