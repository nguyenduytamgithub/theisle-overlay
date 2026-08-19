//! `--replay <file>` dev mode: drive the whole UI from a fixture file with no
//! game running. Each line goes through the exact same parse -> tracker ->
//! events pipeline as real clipboard content.

use std::path::PathBuf;
use std::time::Duration;

use overlay_core::{parse_coordinates, NumberFormat};
use tauri::AppHandle;

use crate::pipeline;

pub fn spawn(app: AppHandle, path: PathBuf) {
    std::thread::spawn(move || {
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                log::error!("replay: cannot read {}: {e}", path.display());
                return;
            }
        };
        // Give the windows a moment to come up before the first sample.
        std::thread::sleep(Duration::from_millis(1500));
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((x, y, z)) = parse_coordinates(line, NumberFormat::Auto) {
                log::info!("replay sample: {x} {y} {z}");
                pipeline::ingest_sample(&app, x, y, z);
            }
            std::thread::sleep(Duration::from_millis(1200));
        }
        log::info!("replay finished");
    });
}
