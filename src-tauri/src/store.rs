//! Waypoint and trail persistence. Port of `app/store.py`.
//!
//! IRON RULE: every coordinate persisted to disk is raw UE centimetres, never
//! pixels. Re-calibrating later must not corrupt saved data.
//!
//! Trails are append-only JSON Lines: one sample per line, flushed
//! immediately. Crash-safe and never rewrites the whole file.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::settings;

fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

// ------------------------------------------------------------- waypoints ---

/// Field names match the Python app's waypoints.json exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Waypoint {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub z: f64,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
}

pub fn load_waypoints() -> Vec<Waypoint> {
    // A corrupt file must never stop the app from starting; individually
    // malformed entries are dropped, the rest kept.
    let Ok(text) = std::fs::read_to_string(settings::waypoints_path()) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    value
        .get("waypoints")
        .and_then(|w| w.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

pub fn save_waypoints(waypoints: &[Waypoint]) -> std::io::Result<()> {
    settings::save_json(
        &settings::waypoints_path(),
        &serde_json::json!({ "version": 1, "waypoints": waypoints }),
    )
}

pub fn new_waypoint(name: &str, x: f64, y: f64, z: f64, color: Option<String>) -> Waypoint {
    Waypoint {
        id: format!("wp_{}", &uuid::Uuid::new_v4().simple().to_string()[..8]),
        name: name.to_string(),
        x,
        y,
        z,
        color,
        created: Some(now_iso()),
    }
}

// ----------------------------------------------------------------- trail ---

/// Appends each sample of the current session to its own JSONL file. The file
/// is created lazily on the first write, so an app run with no samples leaves
/// no empty file behind (and `latest_trail_path` at startup still points at
/// the previous session).
pub struct TrailWriter {
    pub path: PathBuf,
    file: Option<File>,
}

impl Default for TrailWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl TrailWriter {
    pub fn new() -> Self {
        let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        Self {
            path: settings::trails_dir().join(format!("trail_{stamp}.jsonl")),
            file: None,
        }
    }

    fn open(&mut self) -> std::io::Result<&mut File> {
        if self.file.is_none() {
            std::fs::create_dir_all(settings::trails_dir())?;
            self.file = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)?,
            );
        }
        Ok(self.file.as_mut().unwrap())
    }

    fn write_line(&mut self, line: &str) {
        // Trail persistence must never crash the overlay mid-game; failures
        // are logged and the session trail stays in memory regardless.
        let result: std::io::Result<()> = (|| {
            let file = self.open()?;
            file.write_all(line.as_bytes())?;
            file.write_all(b"\n")?;
            file.flush()
        })();
        if let Err(e) = result {
            log::warn!("trail write failed: {e}");
        }
    }

    pub fn add(&mut self, x: f64, y: f64, z: f64) {
        self.write_line(
            &serde_json::json!({ "t": now_iso(), "x": x, "y": y, "z": z }).to_string(),
        );
    }

    pub fn add_break(&mut self) {
        self.write_line(&serde_json::json!({ "t": now_iso(), "break": true }).to_string());
    }
}

/// Read one trail file into a list of DISJOINT segments.
///
/// `break` records split segments. Two consecutive samples can be hours and
/// kilometres apart (sampling is manual), and joining them with a straight
/// line would imply a journey that never happened.
pub fn load_trail(path: &Path) -> Vec<Vec<(f64, f64)>> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut segments: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut current: Vec<(f64, f64)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Skip corrupt lines, keep the rest.
        let Ok(rec) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if rec.get("break").and_then(|b| b.as_bool()).unwrap_or(false) {
            if current.len() > 1 {
                segments.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        } else if let (Some(x), Some(y)) = (
            rec.get("x").and_then(|v| v.as_f64()),
            rec.get("y").and_then(|v| v.as_f64()),
        ) {
            current.push((x, y));
        }
    }
    if current.len() > 1 {
        segments.push(current);
    }
    segments
}

pub fn latest_trail_path() -> Option<PathBuf> {
    let entries = std::fs::read_dir(settings::trails_dir()).ok()?;
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("trail_") && n.ends_with(".jsonl"))
        })
        .collect();
    files.sort();
    files.pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_trail_splits_on_breaks_and_skips_bad_lines() {
        let dir = std::env::temp_dir().join("theisle_overlay_test_trails");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("trail_test.jsonl");
        std::fs::write(
            &path,
            concat!(
                "{\"t\":\"x\",\"x\":1.0,\"y\":2.0,\"z\":0}\n",
                "{\"t\":\"x\",\"x\":3.0,\"y\":4.0,\"z\":0}\n",
                "not json at all\n",
                "{\"t\":\"x\",\"break\":true}\n",
                "{\"t\":\"x\",\"x\":5.0,\"y\":6.0,\"z\":0}\n",
            ),
        )
        .unwrap();
        let segments = load_trail(&path);
        // The single point after the break is dropped (< 2 nodes).
        assert_eq!(segments, vec![vec![(1.0, 2.0), (3.0, 4.0)]]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn waypoint_json_round_trips_python_format() {
        let json = r#"{"id":"wp_ab12cd34","name":"Hang da","x":-231654.353,"y":52099.673,"z":0.0,"color":null,"created":"2026-01-01T00:00:00+07:00"}"#;
        let wp: Waypoint = serde_json::from_str(json).unwrap();
        assert_eq!(wp.id, "wp_ab12cd34");
        assert_eq!(wp.name, "Hang da");
        let back = serde_json::to_value(&wp).unwrap();
        assert_eq!(back["x"], -231654.353);
        assert_eq!(back["color"], serde_json::Value::Null);
    }
}
