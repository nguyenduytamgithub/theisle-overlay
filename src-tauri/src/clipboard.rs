//! Clipboard watcher — the app's ONLY source of position data. Port of
//! `app/clipboard.py`.
//!
//! In game the player presses Tab and clicks "Asset Location"; the game
//! itself copies the coordinates to the Windows clipboard. We only read them
//! back. No memory reads, no hooks, no synthetic input — that is why this
//! tool is safe next to anti-cheat, and also why position only updates when
//! the player copies it MANUALLY.
//!
//! Technique: poll `GetClipboardSequenceNumber()` — a user32 counter that
//! increments on every clipboard write. Calling it does NOT open or lock the
//! clipboard, so it never contends with other apps. Only when the number
//! changes do we actually read the content.
//!
//! Non-coordinate content is SILENTLY ignored — no log, no UI flash. The
//! user's normal copy/paste must feel untouched. The app never WRITES to the
//! clipboard.

use std::time::Duration;

use overlay_core::parse::MAX_CLIPBOARD_LEN;
use overlay_core::{parse_coordinates, NumberFormat};
use tauri::{AppHandle, Manager};
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, GetClipboardSequenceNumber, OpenClipboard,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};

use crate::pipeline;
use crate::settings;
use crate::state::AppState;

const CF_UNICODETEXT: u32 = 13;

/// Read the clipboard text.
///
/// - `Err(())`  — could not OPEN the clipboard (another app holds it): the
///   caller must retry next tick without consuming the sequence number.
/// - `Ok(None)` — opened fine but no text on it: consumed, nothing to do.
fn read_clipboard_text() -> Result<Option<String>, ()> {
    unsafe {
        if OpenClipboard(None).is_err() {
            return Err(());
        }
        let text = (|| {
            let handle: HANDLE = GetClipboardData(CF_UNICODETEXT).ok()?;
            let ptr = GlobalLock(HGLOBAL(handle.0)) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let mut len = 0usize;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            let _ = GlobalUnlock(HGLOBAL(handle.0));
            Some(text)
        })();
        let _ = CloseClipboard();
        Ok(text)
    }
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_seq = unsafe { GetClipboardSequenceNumber() };
        loop {
            let (interval_ms, number_format) = {
                let state = app.state::<AppState>();
                let s = state.settings.lock().unwrap();
                (
                    settings::get_f64(&s, &["poll", "clipboard_ms"], 400.0) as u64,
                    NumberFormat::from_setting(settings::get_str(&s, &["number_format"], "auto")),
                )
            };
            std::thread::sleep(Duration::from_millis(interval_ms.max(100)));

            let seq = unsafe { GetClipboardSequenceNumber() };
            if seq == last_seq {
                continue;
            }
            match read_clipboard_text() {
                Err(()) => continue, // clipboard busy — retry next tick
                Ok(None) => last_seq = seq,
                Ok(Some(text)) => {
                    last_seq = seq;
                    if text.is_empty() || text.chars().count() > MAX_CLIPBOARD_LEN {
                        continue;
                    }
                    if let Some((x, y, z)) = parse_coordinates(&text, number_format) {
                        pipeline::ingest_sample(&app, x, y, z);
                    }
                    // else: not coordinates — silently ignore.
                }
            }
        }
    });
}
