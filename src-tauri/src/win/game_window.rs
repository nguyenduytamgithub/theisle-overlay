//! Locating the game window — read-only system information.
//!
//! The game is identified by PROCESS NAME, not window title: titles change
//! with updates and language. Among candidate windows the largest-area one is
//! chosen. The Toolhelp32 snapshot is a system-wide listing; no handle into
//! the game process is ever opened.

use std::collections::HashSet;

use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetWindow, GetWindowRect, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, GW_OWNER,
};
use windows::core::BOOL;
use windows::Win32::Foundation::POINT;

fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// PIDs of processes whose image name matches. Snapshot only — no handles
/// into the target process.
fn pids_for_image(image_name: &str) -> HashSet<u32> {
    let mut pids = HashSet::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return pids;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                if wide_to_string(&entry.szExeFile).eq_ignore_ascii_case(image_name) {
                    pids.insert(entry.th32ProcessID);
                }
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    pids
}

struct EnumCtx {
    pids: HashSet<u32>,
    /// (area, hwnd as isize)
    found: Vec<(i64, isize)>,
}

extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = unsafe { &mut *(lparam.0 as *mut EnumCtx) };
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if !ctx.pids.contains(&pid) {
            return true.into();
        }
        if !IsWindowVisible(hwnd).as_bool() {
            return true.into();
        }
        // Owned windows are dialogs/tool windows, not the main game surface.
        if GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.is_invalid()) {
            return true.into();
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return true.into();
        }
        let area = (rect.right - rect.left) as i64 * (rect.bottom - rect.top) as i64;
        if area > 0 {
            ctx.found.push((area, hwnd.0 as isize));
        }
    }
    true.into()
}

/// HWND (as isize, thread-portable) of the game's main window, or None while
/// the game is not running.
pub fn find_game_window(image_name: &str) -> Option<isize> {
    let pids = pids_for_image(image_name);
    if pids.is_empty() {
        return None;
    }
    let mut ctx = EnumCtx {
        pids,
        found: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(Some(enum_cb), LPARAM(&mut ctx as *mut EnumCtx as isize));
    }
    ctx.found.iter().max().map(|&(_, hwnd)| hwnd)
}

/// Whether the window is minimized — read-only, no handle into the process.
/// A minimized game must not leave the overlay floating over the desktop.
pub fn is_iconic(hwnd: isize) -> bool {
    unsafe { IsIconic(HWND(hwnd as *mut std::ffi::c_void)).as_bool() }
}

/// (x, y, width, height) of the game's drawing area in PHYSICAL screen
/// pixels. Client area, not GetWindowRect: the client area is what the game
/// actually draws, excluding borders and title bar.
pub fn client_rect_on_screen(hwnd: isize) -> Option<(i32, i32, i32, i32)> {
    let hwnd = HWND(hwnd as *mut std::ffi::c_void);
    unsafe {
        let mut rect = RECT::default();
        GetClientRect(hwnd, &mut rect).ok()?;
        let mut origin = POINT { x: 0, y: 0 };
        if !ClientToScreen(hwnd, &mut origin).as_bool() {
            return None;
        }
        Some((origin.x, origin.y, rect.right, rect.bottom))
    }
}
