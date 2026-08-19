//! First-run data download + conversion. Port of `tools/fetch_data.py`.
//!
//! Why DOWNLOAD instead of bundling: the source data belongs to others (the
//! basemap is VulnonaMAP's, derived from Afterthought LLC game assets). A
//! user fetching a personal copy to their own machine is a different thing
//! from the app author redistributing that database. See sources.json.
//!
//! AXIS CONVENTION — the easiest place to slip in this whole file:
//! ```text
//! ours (and Vulnona's):  gameX = Lat -> VERTICAL,  gameY = Long -> HORIZONTAL
//! myislemap's:           ueX/x = Long,             ueY/y = Lat   (SWAPPED)
//! ```
//! Verified: myislemap's 'x' value range overflows the gameX bounds but fits
//! the gameY bounds exactly.
//!
//! The scrapers are regex over third-party JS and WILL break some day —
//! that is why failures are per-source and the app keeps working with
//! whatever succeeded ("map only" is a valid outcome).

use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::settings;

pub const MAP_VERSION: &str = "Gateway_v0.21.7";
const UA: &str = "theisle-overlay/2.0 (personal use; contact via github)";

fn vulnona_base() -> String {
    format!("https://vulnona.com/game/map/map/{MAP_VERSION}")
}

/// Tier 1 for the minimap, tier 3 for the full map. Tier 4 (7800 px) decodes
/// to ~244 MB and is not fetched.
const BASEMAP_TIERS: [(u8, &str); 2] = [(1, "minimap"), (3, "fullmap")];

// --- myislemap's SVG coordinate system (sanctuary/migration zones) ---------
const SVG_W: f64 = 1000.0;
const SVG_H: f64 = 1003.0;
const SPAN_X: f64 = 1116.0;
const SPAN_Y: f64 = 1112.0;
const MIN_X: f64 = -607.0;
const MIN_Y: f64 = -505.0;

/// myislemap SVG coords -> (gameX, gameY) in cm.
fn svg_to_world(sx: f64, sy: f64) -> (f64, f64) {
    let game_x = (sy / SVG_H * SPAN_X + MIN_X) * 1000.0;
    let game_y = (sx / SVG_W * SPAN_Y + MIN_Y) * 1000.0;
    (game_x, game_y)
}

// ---------------------------------------------------------------- parsers ---

// myislemap POI records are flat, non-nested JS objects.
static RE_POI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)\{[^{}]*?key:\s*"(?P<key>[a-z_]+)"[^{}]*?ueX:\s*(?P<uex>-?[\d.]+)[^{}]*?ueY:\s*(?P<uey>-?[\d.]+)[^{}]*?\}"#,
    )
    .unwrap()
});

// NOTE: depends on the remote file's exact two-space indentation, like the
// original. Upstream reformatting yields zero zones — fail-soft handles it.
static RE_ZONE_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?sm)^\s{2}(?P<name>sanctuary|migration|patrol):\s*\{(?P<body>.*?)^\s{2}\},")
        .unwrap()
});
static RE_CIRCLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"\{\s*type:\s*"circle",\s*cx:\s*(-?[\d.]+),\s*cy:\s*(-?[\d.]+),\s*r:\s*(-?[\d.]+),\s*label:\s*"([^"]*)""#,
    )
    .unwrap()
});
static RE_POLYGON: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\{\s*type:\s*"polygon",\s*points:\s*"([^"]+)",\s*label:\s*"([^"]*)""#).unwrap()
});
static RE_VULNONA_REC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^text\twater\t(?P<name>[^\t\n]+)[^\n]*\n(?P<x>-?[\d.]+),(?P<y>-?[\d.]+),")
        .unwrap()
});

/// Point POIs (salt licks, mud wallows...) from map-data.js.
fn parse_point_pois(js: &str, wanted: &[&str]) -> std::collections::HashMap<String, Vec<Value>> {
    let mut out: std::collections::HashMap<String, Vec<Value>> =
        wanted.iter().map(|k| (k.to_string(), Vec::new())).collect();
    for caps in RE_POI.captures_iter(js) {
        let key = &caps["key"];
        if let Some(list) = out.get_mut(key) {
            // Their ueX is Long (our gameY), their ueY is Lat (our gameX).
            let (Ok(uex), Ok(uey)) = (caps["uex"].parse::<f64>(), caps["uey"].parse::<f64>())
            else {
                continue;
            };
            list.push(json!({ "label": "", "x": uey, "y": uex }));
        }
    }
    out
}

/// Zones (sanctuary, migration) from MAP_OVERLAYS.
fn parse_zones(js: &str, wanted: &[&str]) -> std::collections::HashMap<String, Vec<Value>> {
    let mut out = std::collections::HashMap::new();
    for block in RE_ZONE_BLOCK.captures_iter(js) {
        let name = &block["name"];
        if !wanted.contains(&name) {
            continue;
        }
        let body = &block["body"];
        let mut zones = Vec::new();

        for c in RE_CIRCLE.captures_iter(body) {
            let (Ok(cx), Ok(cy), Ok(r)) = (
                c[1].parse::<f64>(),
                c[2].parse::<f64>(),
                c[3].parse::<f64>(),
            ) else {
                continue;
            };
            let (gx, gy) = svg_to_world(cx, cy);
            // Radius: SVG units -> metres along the horizontal axis.
            let radius_m = r / SVG_W * SPAN_Y * 1000.0 / 100.0;
            zones.push(json!({
                "shape": "circle", "label": &c[4], "x": gx, "y": gy, "radius_m": radius_m
            }));
        }

        for p in RE_POLYGON.captures_iter(body) {
            let mut verts = Vec::new();
            for pair in p[1].split_whitespace() {
                let Some((sx, sy)) = pair.split_once(',') else {
                    continue;
                };
                let (Ok(sx), Ok(sy)) = (sx.parse::<f64>(), sy.parse::<f64>()) else {
                    continue;
                };
                let (gx, gy) = svg_to_world(sx, sy);
                verts.push(json!([gx, gy]));
            }
            if !verts.is_empty() {
                zones.push(json!({ "shape": "polygon", "label": &p[2], "points": verts }));
            }
        }

        out.insert(name.to_string(), zones);
    }
    out
}

/// AI spawn zones: plain JSON after the '='. Coordinates are raw UE cm.
fn parse_ai_zones(js: &str) -> Result<Vec<Value>, String> {
    let eq = js.find('=').ok_or("no '=' in ai zones file")?;
    let payload = js[eq + 1..].trim().trim_end_matches(';');
    let parsed: Vec<Value> = serde_json::from_str(payload).map_err(|e| e.to_string())?;
    let mut zones = Vec::new();
    for z in parsed {
        let loc = z.get("location").cloned().unwrap_or(json!({}));
        let mut species: Vec<String> = z
            .get("configs")
            .and_then(|c| c.as_array())
            .map(|configs| {
                configs
                    .iter()
                    .filter_map(|c| c.get("name").and_then(|n| n.as_str()))
                    .filter(|n| !n.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        species.sort();
        species.dedup();
        // Same swap convention: their x is Long, their y is Lat.
        let verts: Vec<Value> = z
            .get("points")
            .and_then(|p| p.as_array())
            .map(|pts| {
                pts.iter()
                    .filter_map(|p| {
                        Some(json!([p.get("y")?.as_f64()?, p.get("x")?.as_f64()?]))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let label = if species.is_empty() {
            z.get("label").and_then(|l| l.as_str()).unwrap_or("").to_string()
        } else {
            species.join(", ")
        };
        zones.push(json!({
            "shape": if verts.is_empty() { "point" } else { "polygon" },
            "label": label,
            "species": species,
            "x": loc.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0),
            "y": loc.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0),
            "points": verts,
        }));
    }
    Ok(zones)
}

/// Named water sources from Vulnona's data_1.txt (thousand-cm units).
fn parse_water(txt: &str) -> Vec<Value> {
    RE_VULNONA_REC
        .captures_iter(txt)
        .filter_map(|m| {
            Some(json!({
                "label": m["name"].trim(),
                "x": m["x"].parse::<f64>().ok()? * 1000.0,
                "y": m["y"].parse::<f64>().ok()? * 1000.0,
            }))
        })
        .collect()
}

// ------------------------------------------------------------------ fetch ---

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchProgress {
    pub file: String,
    pub index: usize,
    pub total: usize,
    /// "downloading" | "done" | "skipped" | "error"
    pub status: &'static str,
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchFinished {
    pub ok: bool,
    pub basemap_ok: bool,
    pub pois_ok: bool,
    pub error: Option<String>,
}

fn emit_progress(app: &AppHandle, p: FetchProgress) {
    let _ = app.emit("fetch://progress", p);
}

fn download(client: &reqwest::blocking::Client, url: &str, dest: &Path, force: bool) -> Result<bool, String> {
    if dest.exists() && !force {
        return Ok(false);
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.bytes())
        .map_err(|e| e.to_string())?;
    std::fs::write(dest, &bytes).map_err(|e| e.to_string())?;
    Ok(true)
}

/// The whole fetch + convert, blocking. Runs on a worker thread; progress and
/// completion arrive as events.
pub fn run(app: &AppHandle, force: bool) -> FetchFinished {
    let client = match reqwest::blocking::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(90))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return FetchFinished {
                ok: false,
                basemap_ok: false,
                pois_ok: false,
                error: Some(e.to_string()),
            }
        }
    };

    let base = vulnona_base();
    let sources: Vec<(String, String, std::path::PathBuf, bool)> = {
        let mut v = Vec::new();
        for (tier, role) in BASEMAP_TIERS {
            v.push((
                format!("{role}.webp"),
                format!("{base}/base/{tier}.webp"),
                settings::basemap_dir().join(format!("{role}.webp")),
                force, // basemap: skip when present unless forced
            ));
        }
        for (name, url) in [
            ("map-data.js", "https://myislemap.com/map-data.js".to_string()),
            (
                "map-ai-spawn-zones.js",
                "https://myislemap.com/map-ai-spawn-zones.js".to_string(),
            ),
            ("data_1.txt", format!("{base}/data_1.txt")),
            ("dat.txt", "https://vulnona.com/game/map/dat.txt".to_string()),
        ] {
            v.push((name.to_string(), url, settings::cache_dir().join(name), true));
        }
        v
    };

    let total = sources.len();
    let mut basemap_ok = true;
    let mut scrape_sources_ok = true;
    for (index, (name, url, dest, force_this)) in sources.iter().enumerate() {
        emit_progress(
            app,
            FetchProgress {
                file: name.clone(),
                index,
                total,
                status: "downloading",
                error: None,
            },
        );
        match download(&client, url, dest, *force_this) {
            Ok(true) => emit_progress(
                app,
                FetchProgress {
                    file: name.clone(),
                    index,
                    total,
                    status: "done",
                    error: None,
                },
            ),
            Ok(false) => emit_progress(
                app,
                FetchProgress {
                    file: name.clone(),
                    index,
                    total,
                    status: "skipped",
                    error: None,
                },
            ),
            Err(e) => {
                log::warn!("fetch {name} failed: {e}");
                if name.ends_with(".webp") {
                    // dest may still exist from before; only fatal if missing.
                    if !dest.exists() {
                        basemap_ok = false;
                    }
                } else if !dest.exists() {
                    scrape_sources_ok = false;
                }
                emit_progress(
                    app,
                    FetchProgress {
                        file: name.clone(),
                        index,
                        total,
                        status: "error",
                        error: Some(e),
                    },
                );
            }
        }
    }

    // Convert whatever made it to disk.
    let pois_ok = scrape_sources_ok && convert(app).is_ok();

    let finished = FetchFinished {
        ok: basemap_ok && pois_ok,
        basemap_ok,
        pois_ok,
        error: None,
    };
    let _ = app.emit("fetch://finished", finished.clone());
    finished
}

fn convert(_app: &AppHandle) -> Result<(), String> {
    let read = |name: &str| {
        std::fs::read_to_string(settings::cache_dir().join(name)).map_err(|e| e.to_string())
    };
    let map_data = read("map-data.js")?;
    let ai_data = read("map-ai-spawn-zones.js")?;
    let water_txt = read("data_1.txt")?;

    let points = parse_point_pois(&map_data, &["saltrock", "mudwallow"]);
    let zones = parse_zones(&map_data, &["sanctuary", "migration"]);
    let ai_zones = parse_ai_zones(&ai_data).unwrap_or_default();
    let water = parse_water(&water_txt);

    let pois = json!({
        "version": 1,
        "map": MAP_VERSION,
        "units": "ue_cm",
        "_axis": "x = Lat (truc doc), y = Long (truc ngang)",
        "layers": {
            "water": { "kind": "point", "items": water },
            "saltlick": { "kind": "point", "items": points.get("saltrock").cloned().unwrap_or_default() },
            "mudwallow": { "kind": "point", "items": points.get("mudwallow").cloned().unwrap_or_default() },
            "sanctuary": { "kind": "zone", "items": zones.get("sanctuary").cloned().unwrap_or_default() },
            "migration": { "kind": "zone", "items": zones.get("migration").cloned().unwrap_or_default() },
            "food": { "kind": "zone", "items": ai_zones },
        },
    });
    settings::save_json(&settings::pois_path(), &pois).map_err(|e| e.to_string())?;

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let base = vulnona_base();
    settings::save_json(
        &settings::sources_path(),
        &json!({
            "fetched": today,
            "map_version": MAP_VERSION,
            "basemap": {
                "url": format!("{base}/base/{{tier}}.webp"),
                "tiers": [1, 3],
                "credit": "VulnonaMAP (Coco.N). Composite of in-game screenshots. Imagery (c) Afterthought LLC (The Isle).",
            },
            "poi_sources": [
                { "layers": ["saltlick", "mudwallow", "sanctuary", "migration"],
                  "url": "https://myislemap.com/map-data.js", "credit": "myislemap.com" },
                { "layers": ["food"], "url": "https://myislemap.com/map-ai-spawn-zones.js",
                  "credit": "myislemap.com (datamined AI spawn zones)" },
                { "layers": ["water"], "url": format!("{base}/data_1.txt"),
                  "credit": "VulnonaMAP (Coco.N)" },
            ],
            "note": "Unaffiliated with Afterthought LLC. Personal-use local copy.",
        }),
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poi_regex_swaps_axes_on_import() {
        let js = r#"
          { key: "saltrock", name: "A", ueX: 52099.6, ueY: -231654.3, other: 1 },
          { key: "mudwallow", name: "B", ueX: 1.0, ueY: 2.0 },
          { key: "ignored_key", ueX: 9.0, ueY: 9.0 },
        "#;
        let out = parse_point_pois(js, &["saltrock", "mudwallow"]);
        // their ueY (Lat) -> our x; their ueX (Long) -> our y
        assert_eq!(out["saltrock"][0]["x"], -231654.3);
        assert_eq!(out["saltrock"][0]["y"], 52099.6);
        assert_eq!(out["mudwallow"].len(), 1);
    }

    #[test]
    fn zone_block_requires_two_space_indent() {
        let js = "const MAP_OVERLAYS = {\n  sanctuary: {\n    items: [ { type: \"circle\", cx: 500.0, cy: 501.5, r: 10.0, label: \"Mid\" } ]\n  },\n  migration: {\n    items: [ { type: \"polygon\", points: \"0,0 1000,0 1000,1003\", label: \"Sweep\" } ]\n  },\n};";
        let out = parse_zones(js, &["sanctuary", "migration"]);
        let mid = &out["sanctuary"][0];
        assert_eq!(mid["shape"], "circle");
        // SVG centre (500, 501.5): gameX = (0.5*1116 - 607)*1000 = -49000 cm,
        // gameY = (0.5*1112 - 505)*1000 = 51000 cm.
        assert!((mid["x"].as_f64().unwrap() - -49000.0).abs() < 1.0);
        assert!((mid["y"].as_f64().unwrap() - 51000.0).abs() < 1.0);
        let poly = &out["migration"][0];
        assert_eq!(poly["points"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn ai_zones_swap_and_join_species() {
        let js = r#"window.AI_ZONES = [
          { "location": {"x": 106000.0, "y": 254000.0},
            "points": [{"x": 1.0, "y": 2.0}, {"x": 3.0, "y": 4.0}, {"x": 5.0, "y": 6.0}],
            "configs": [{"name": "Deer"}, {"name": "Chickens"}] }
        ];"#;
        let zones = parse_ai_zones(js).unwrap();
        assert_eq!(zones[0]["x"], 254000.0, "their location.y (Lat) is our x");
        assert_eq!(zones[0]["y"], 106000.0);
        assert_eq!(zones[0]["label"], "Chickens, Deer");
        assert_eq!(zones[0]["points"][0], json!([2.0, 1.0]));
    }

    /// Runs the ported parsers against the REAL files the old Python app
    /// cached, comparing item counts with what fetch_data.py produced
    /// (water 27, saltlick 24, mudwallow 36, sanctuary 7, migration 12,
    /// food 52). Ignored by default because it needs those files on disk:
    /// `cargo test -- --ignored parse_real_cache`
    #[test]
    #[ignore]
    fn parse_real_cache_files_matches_python_output() {
        let cache = crate::settings::cache_dir();
        let read = |name: &str| std::fs::read_to_string(cache.join(name)).unwrap();
        let map_data = read("map-data.js");
        let ai_data = read("map-ai-spawn-zones.js");
        let water_txt = read("data_1.txt");

        let points = parse_point_pois(&map_data, &["saltrock", "mudwallow"]);
        let zones = parse_zones(&map_data, &["sanctuary", "migration"]);
        let ai = parse_ai_zones(&ai_data).unwrap();
        let water = parse_water(&water_txt);

        assert_eq!(water.len(), 27, "water");
        assert_eq!(points["saltrock"].len(), 24, "saltlick");
        assert_eq!(points["mudwallow"].len(), 36, "mudwallow");
        assert_eq!(zones["sanctuary"].len(), 7, "sanctuary");
        assert_eq!(zones["migration"].len(), 12, "migration");
        assert_eq!(ai.len(), 52, "food");
    }

    #[test]
    fn water_parser_scales_thousands_to_cm() {
        let txt = "text\twater\tDam Lake\textra\n-267.0,79.0,\nother line\n";
        let water = parse_water(txt);
        assert_eq!(water[0]["label"], "Dam Lake");
        assert_eq!(water[0]["x"], -267000.0);
        assert_eq!(water[0]["y"], 79000.0);
    }
}
