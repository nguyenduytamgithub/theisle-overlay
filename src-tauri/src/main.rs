// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

fn main() {
    // `--replay <file>` (or THEISLE_REPLAY=<file>, easier to thread through
    // `npm run tauri dev`): drive the UI from a fixture, no game needed.
    let mut args = std::env::args().skip(1);
    let mut replay: Option<PathBuf> = std::env::var_os("THEISLE_REPLAY").map(PathBuf::from);
    while let Some(arg) = args.next() {
        if arg == "--replay" {
            replay = args.next().map(PathBuf::from);
        }
    }
    theisle_overlay_lib::run(replay);
}
