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

use tauri::WebviewWindow;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2_19, ICoreWebView2_3, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
};
// NOT windows::core — webview2-com's interfaces are generated against
// windows 0.61, so `cast` comes from that version's Interface trait.
use windows_core::Interface;

/// Freeze a HIDDEN window's webview and release renderer memory. Fail-soft:
/// on any error the window simply keeps its memory (old behaviour).
pub fn suspend(window: &WebviewWindow) {
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

/// Undo `suspend`. Called before showing the window again. (Making the
/// controller visible auto-resumes too; the explicit calls are belt and
/// braces and restore the normal memory target.)
pub fn resume(window: &WebviewWindow) {
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
