//! Win32 calls. Port of `app/winapi.py` — including its safety contract.
//!
//! EVERY FUNCTION IN THIS MODULE IS ON THE ALLOWED LIST:
//!   - Read-only: EnumWindows, GetWindowRect, GetClientRect, ClientToScreen,
//!     GetWindowThreadProcessId, the Toolhelp32 process snapshot, and
//!     GetClipboardSequenceNumber / clipboard READS (in clipboard.rs).
//!   - Writes: ONLY to this app's own windows (SetWindowLongPtr, SetWindowPos).
//!   - RegisterHotKey (in hotkeys.rs) — a documented, cooperative OS API, not
//!     a keyboard hook.
//!
//! The game runs kernel-level Easy Anti-Cheat. This app is safe only because
//! it never touches the game process. ABSOLUTELY NEVER add: OpenProcess /
//! ReadProcessMemory aimed at the game, SetParent into the game window, DLL
//! injection, DirectX hooks, synthetic input (SendInput / keybd_event /
//! PostMessage to the game), packet capture, or low-level keyboard hooks
//! (SetWindowsHookEx). CI greps for those names and fails the build.

pub mod game_window;
pub mod overlay;
