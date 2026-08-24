//! Style-bit operations on THE APP'S OWN overlay window only. Port of the
//! own-window half of `app/winapi.py`.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
};

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut std::ffi::c_void)
}

/// Toggle letting the mouse pass through the overlay to the game underneath.
///
/// Flips the WS_EX_TRANSPARENT bit on the live HWND — never recreate the
/// window at runtime (a recreate causes a visible flash and loses topmost).
pub fn set_click_through(raw: isize, enabled: bool) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd(raw), GWL_EXSTYLE);
        let ex = if enabled {
            ex | WS_EX_TRANSPARENT.0 as isize
        } else {
            ex & !(WS_EX_TRANSPARENT.0 as isize)
        };
        SetWindowLongPtrW(hwnd(raw), GWL_EXSTYLE, ex);
        let _ = SetWindowPos(
            hwnd(raw),
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// Unconditional HWND_TOPMOST — for the moment a hidden overlay is shown
/// again. `ensure_topmost` only reads the WS_EX_TOPMOST style bit, which
/// stays SET while another topmost window (the game flipping to borderless /
/// exclusive fullscreen, a Steam or Discord overlay) sits above us inside
/// the topmost band; ShowWindow then restores the OLD z-position, i.e.
/// behind the game, and the checked variant never repairs it. One
/// SetWindowPos per show is cheap; the periodic poll keeps the style check.
pub fn force_topmost(raw: isize) {
    unsafe {
        let _ = SetWindowPos(
            hwnd(raw),
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// Re-assert topmost if the STYLE BIT was lost — the game grabs it back on
/// focus changes. Blind to z-order changes within the topmost band; see
/// `force_topmost` for the on-show case.
///
/// Checks before setting: calling SetWindowPos unconditionally every 2 s
/// forces a needless DWM repaint each time.
pub fn ensure_topmost(raw: isize) {
    unsafe {
        if GetWindowLongPtrW(hwnd(raw), GWL_EXSTYLE) & WS_EX_TOPMOST.0 as isize == 0 {
            let _ = SetWindowPos(
                hwnd(raw),
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
}

/// Belt-and-braces after creating the minimap window: assert the styles that
/// keep the overlay from ever taking keyboard focus or appearing in the
/// taskbar, regardless of what the windowing library set.
pub fn assert_overlay_styles(raw: isize) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd(raw), GWL_EXSTYLE);
        let wanted = ex | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize;
        if wanted != ex {
            SetWindowLongPtrW(hwnd(raw), GWL_EXSTYLE, wanted);
        }
    }
}
