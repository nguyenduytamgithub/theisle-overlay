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
// This is myislemap's own frame for POI IMPORT (producing world cm) — it is
// unrelated to which basemap imagery the user views.
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
// islemaps.com animal-sighting records inside their map JS bundle, minified
// as {lat:390.427,lng:147.898,info:"Boar"} (whitespace tolerated).
static RE_SIGHTING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"\{\s*lat:\s*(?P<lat>-?[\d.]+)\s*,\s*lng:\s*(?P<lng>-?[\d.]+)\s*,\s*info:\s*"(?P<info>[^"]+)"\s*\}"#,
    )
    .unwrap()
});

// Vulnona text records, all three kinds share one shape:
//   line 1: text<TAB>kind<TAB>name[<TAB>size-hints]
//   line 2: x,y,displaytext,           (thousand-cm units)
// The CLEAN name is column 3 of line 1 (the display text on line 2 carries
// <br>/<s> markup). Names starting with ':' are upstream comments — skipped.
static RE_VULNONA_REC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^text\t(?P<kind>water|area|land)\t(?P<name>[^\t\n]+)[^\n]*\n(?P<x>-?[\d.]+),(?P<y>-?[\d.]+),",
    )
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

/// The species that belong on the "animal" layer. islemaps' sighting records
/// also carry "Salt" (salt licks — the myislemap saltlick layer already
/// covers those); anything not on this list is skipped.
const ANIMAL_SPECIES: [&str; 9] = [
    "Boar", "Bunny", "Chicken", "Crab", "Deer", "Frog", "Goat", "Teno", "Turtle",
];

/// Animal spawn points from islemaps.com's map bundle.
///
/// Their leaflet frame: lat = -gameX/1000, lng = gameY/1000 — the NEGATED X
/// (verified against direction-named POIs and confirmed by verify_data's
/// on-land check). Do not "simplify" the sign.
fn parse_islemaps_animals(js: &str) -> Vec<Value> {
    RE_SIGHTING
        .captures_iter(js)
        .filter_map(|m| {
            let info = &m["info"];
            if !ANIMAL_SPECIES.contains(&info) {
                return None;
            }
            Some(json!({
                "label": info,
                "x": -m["lat"].parse::<f64>().ok()? * 1000.0,
                "y": m["lng"].parse::<f64>().ok()? * 1000.0,
            }))
        })
        .collect()
}

/// Named records of one kind ("water" | "area" | "land") from Vulnona's
/// data_1.txt.
fn parse_vulnona_text(txt: &str, kind: &str) -> Vec<Value> {
    RE_VULNONA_REC
        .captures_iter(txt)
        .filter(|m| &m["kind"] == kind)
        .filter_map(|m| {
            let name = m["name"].trim();
            if name.starts_with(':') {
                return None; // upstream comment record
            }
            Some(json!({
                "label": name,
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

// ------------------------------------------------- islemaps.com basemaps ---
// Alternative Gateway imagery, fetched ON DEMAND when the user first selects
// it in Settings (never part of the first-run bundle, never vendored). Both
// styles are single 2500x2500 PNGs; the matching calibration is embedded in
// overlay-core (calibration_islemaps.json).

/// Both islemaps images must decode to exactly this square size — the
/// embedded calibration is derived for it. A different upstream re-export is
/// rejected rather than silently mis-calibrated.
pub const ISLEMAPS_EXPECTED_DIM: u32 = 2500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslemapsVariant {
    Light,
    Dark,
}

impl IslemapsVariant {
    /// The variant a MapSource selects, or None for vulnona.
    pub fn for_source(source: overlay_core::MapSource) -> Option<IslemapsVariant> {
        match source {
            overlay_core::MapSource::Vulnona => None,
            overlay_core::MapSource::IslemapsLight => Some(IslemapsVariant::Light),
            overlay_core::MapSource::IslemapsDark => Some(IslemapsVariant::Dark),
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            IslemapsVariant::Light => "light.png",
            IslemapsVariant::Dark => "dark.png",
        }
    }

    pub fn url(self) -> &'static str {
        match self {
            IslemapsVariant::Light => "https://www.islemaps.com/map/map-light.png",
            IslemapsVariant::Dark => "https://www.islemaps.com/map/map-dark.png",
        }
    }

    pub fn dest(self) -> std::path::PathBuf {
        settings::islemaps_dir().join(self.file_name())
    }

    /// Key inside islemaps meta.json ("light" | "dark").
    fn meta_key(self) -> &'static str {
        match self {
            IslemapsVariant::Light => "light",
            IslemapsVariant::Dark => "dark",
        }
    }

    fn progress_label(self) -> String {
        format!("islemaps/{}", self.file_name())
    }
}

/// PNG width/height straight from the IHDR chunk (8-byte signature, then
/// 4-byte length + "IHDR" + width/height as big-endian u32) — no image crate
/// needed for an integrity check.
fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < 24 || bytes[..8] != SIG || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let be = |i: usize| u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap());
    Some((be(16), be(20)))
}

fn islemaps_meta_path() -> std::path::PathBuf {
    settings::islemaps_dir().join("meta.json")
}

fn read_islemaps_meta() -> Value {
    std::fs::read_to_string(islemaps_meta_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslemapsOutcome {
    Downloaded,
    NotModified,
    AlreadyPresent,
}

/// Download one islemaps image. `force` re-checks an existing file with
/// If-None-Match (their server sends ETag + must-revalidate); without it an
/// existing file is left alone. A bad or resized upstream image is rejected
/// and the old file kept — `.tmp` + rename means a partial download can never
/// clobber a good file.
pub fn fetch_islemaps(
    client: &reqwest::blocking::Client,
    variant: IslemapsVariant,
    force: bool,
) -> Result<IslemapsOutcome, String> {
    let dest = variant.dest();
    let exists = dest.exists();
    if exists && !force {
        return Ok(IslemapsOutcome::AlreadyPresent);
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut req = client.get(variant.url());
    let meta = read_islemaps_meta();
    if exists {
        if let Some(etag) = meta[variant.meta_key()]["etag"].as_str() {
            req = req.header("If-None-Match", etag);
        }
    }
    let resp = req.send().map_err(|e| e.to_string())?;
    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(IslemapsOutcome::NotModified);
    }
    let resp = resp.error_for_status().map_err(|e| e.to_string())?;
    let etag = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let bytes = resp.bytes().map_err(|e| e.to_string())?;

    match png_dimensions(&bytes) {
        Some((ISLEMAPS_EXPECTED_DIM, ISLEMAPS_EXPECTED_DIM)) => {}
        Some((w, h)) => {
            return Err(format!(
                "unexpected image size {w}x{h} (want {ISLEMAPS_EXPECTED_DIM}x{ISLEMAPS_EXPECTED_DIM}) — \
                 upstream re-export would need a new calibration"
            ));
        }
        None => return Err("response is not a PNG".to_string()),
    }

    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;

    let mut meta = meta;
    meta[variant.meta_key()] = json!({
        "etag": etag,
        "fetched": chrono::Local::now().format("%Y-%m-%d").to_string(),
    });
    if let Err(e) = settings::save_json(&islemaps_meta_path(), &meta) {
        log::warn!("islemaps meta.json save failed: {e}"); // cosmetic — next force re-downloads
    }
    Ok(IslemapsOutcome::Downloaded)
}

/// The fresh-water overlay islemaps paints over its basemap: a transparent
/// 2500x2500 PNG in the SAME frame as the islemaps basemap (verified: their
/// site passes one shared bounds object to every imageOverlay). Because the
/// frame is known, the overlay can be re-projected onto ANY basemap; the px
/// bounds come from get_map_info so the frontend never transforms.
pub const ISLEMAPS_FRESHWATER_URL: &str = "https://www.islemaps.com/layers/water.png";

pub fn freshwater_dest() -> std::path::PathBuf {
    settings::islemaps_dir().join("freshwater.png")
}

/// Locate and cache the islemaps.com JS chunk that embeds the animal-sighting
/// records, as `cache\islemaps-sightings.js`. The chunk file name is a build
/// hash that changes on every site deploy, so this is a two-step fetch:
/// homepage -> enumerate /_nuxt/*.js -> first chunk with >= 20 sighting
/// records wins. Like every scraper here it WILL break some day — callers
/// treat failure as "no animal layer this time", nothing else.
fn fetch_islemaps_sightings(client: &reqwest::blocking::Client, force: bool) -> Result<bool, String> {
    let dest = settings::cache_dir().join("islemaps-sightings.js");
    if dest.exists() && !force {
        return Ok(false);
    }
    let html = client
        .get("https://www.islemaps.com/")
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
        .map_err(|e| e.to_string())?;
    let re_chunk = Regex::new(r"_nuxt/[A-Za-z0-9_.-]+\.js").unwrap();
    let mut chunks: Vec<&str> = Vec::new();
    for m in re_chunk.find_iter(&html) {
        if !chunks.contains(&m.as_str()) {
            chunks.push(m.as_str());
        }
    }
    if chunks.is_empty() {
        return Err("no /_nuxt/*.js chunks in islemaps.com homepage (site layout changed?)".into());
    }
    for chunk in chunks.iter().take(40) {
        let js = match client
            .get(format!("https://www.islemaps.com/{chunk}"))
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.text())
        {
            Ok(js) => js,
            Err(_) => continue, // one broken chunk must not sink the search
        };
        if RE_SIGHTING.captures_iter(&js).take(20).count() >= 20 {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&dest, &js).map_err(|e| e.to_string())?;
            return Ok(true);
        }
    }
    Err("no islemaps chunk carries sighting records (site layout changed?)".into())
}

/// `fetch_islemaps` with its own client and `fetch://progress` events —
/// the single-file path behind `set_basemap_source`.
pub fn fetch_islemaps_with_events(
    app: &AppHandle,
    variant: IslemapsVariant,
    force: bool,
) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())?;
    let file = variant.progress_label();
    let progress = |status, error| FetchProgress {
        file: file.clone(),
        index: 0,
        total: 1,
        status,
        error,
    };
    emit_progress(app, progress("downloading", None));
    match fetch_islemaps(&client, variant, force) {
        Ok(IslemapsOutcome::Downloaded) => {
            emit_progress(app, progress("done", None));
            Ok(())
        }
        Ok(_) => {
            emit_progress(app, progress("skipped", None));
            Ok(())
        }
        Err(e) => {
            log::warn!("fetch {file} failed: {e}");
            emit_progress(app, progress("error", Some(e.clone())));
            Err(e)
        }
    }
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

    // Optional islemaps extras: the animal-sighting chunk (feeds the
    // "animal" POI layer) and the fresh-water overlay PNG. Fail-soft: an
    // error only means that layer is absent this round — never affects
    // pois_ok/basemap_ok.
    optional_step(app, "islemaps-sightings.js", || {
        fetch_islemaps_sightings(&client, force)
    });
    optional_step(app, "islemaps/freshwater.png", || {
        download(&client, ISLEMAPS_FRESHWATER_URL, &freshwater_dest(), force)
    });

    // Refresh any islemaps imagery the user already downloaded (conditional
    // via ETag, so an unchanged upstream is one cheap 304). Refresh-only:
    // never fetched here for the first time, failures never affect
    // basemap_ok — vulnona remains the gating imagery.
    for variant in [IslemapsVariant::Light, IslemapsVariant::Dark] {
        if variant.dest().exists() {
            let _ = fetch_islemaps_with_events(app, variant, force);
        }
    }

    // Convert whatever made it to disk.
    let pois_ok = scrape_sources_ok && convert().is_ok();

    let finished = FetchFinished {
        ok: basemap_ok && pois_ok,
        basemap_ok,
        pois_ok,
        error: None,
    };
    let _ = app.emit("fetch://finished", finished.clone());
    finished
}

/// Bump when convert() emits new layers/fields — ensure_pois_current()
/// re-converts old on-disk data (offline, from the cache) on upgrade.
/// v3: optional "animal" layer (islemaps.com sightings).
pub const POIS_VERSION: u64 = 3;

/// Re-run the cache -> pois_gateway.json conversion when the on-disk file
/// predates the current POIS_VERSION and the cached sources are still
/// present. Existing users get the new layers on first launch after an app
/// update, with no network involved.
pub fn ensure_pois_current() {
    let on_disk_version = std::fs::read_to_string(settings::pois_path())
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("version").and_then(|x| x.as_u64()))
        .unwrap_or(0);
    if on_disk_version >= POIS_VERSION {
        return;
    }
    let cache_ok = ["map-data.js", "map-ai-spawn-zones.js", "data_1.txt"]
        .iter()
        .all(|name| settings::cache_dir().join(name).exists());
    if !cache_ok {
        return; // first-run / re-download flows will produce current data
    }
    match convert() {
        Ok(()) => log::info!("pois upgraded to version {POIS_VERSION} from cache"),
        Err(e) => log::warn!("pois upgrade failed: {e}"),
    }
}

/// One fail-soft OPTIONAL fetch with progress events — used for sources whose
/// absence only hides a single layer (never flips the ok flags).
fn optional_step(
    app: &AppHandle,
    file: &str,
    fetch: impl FnOnce() -> Result<bool, String>,
) {
    let progress = |status, error| FetchProgress {
        file: file.to_string(),
        index: 0,
        total: 1,
        status,
        error,
    };
    emit_progress(app, progress("downloading", None));
    match fetch() {
        Ok(true) => emit_progress(app, progress("done", None)),
        Ok(false) => emit_progress(app, progress("skipped", None)),
        Err(e) => {
            log::warn!("fetch {file} failed: {e}");
            emit_progress(app, progress("error", Some(e)));
        }
    }
}

/// Quiet background top-up for data an app UPDATE added that the offline
/// reconvert cannot produce because the old version never cached its source.
/// Currently: the islemaps animal sightings and the fresh-water overlay. Runs
/// once per missing source (the cached file ends the retries), fail-soft and
/// silent — on success both webviews pick the new data up live via
/// fetch://finished.
pub fn spawn_topup(app: &AppHandle) {
    let need_sightings = !settings::cache_dir().join("islemaps-sightings.js").exists();
    let need_freshwater = !freshwater_dest().exists();
    if !need_sightings && !need_freshwater {
        return;
    }
    // Not on first run — the first-run flow fetches everything anyway.
    if !settings::pois_path().exists() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let Ok(client) = reqwest::blocking::Client::builder()
            .user_agent(UA)
            .timeout(Duration::from_secs(90))
            .build()
        else {
            return;
        };
        let mut got_any = false;
        if need_sightings {
            match fetch_islemaps_sightings(&client, false) {
                Ok(_) => got_any |= convert().is_ok(),
                Err(e) => log::info!("islemaps sightings top-up skipped: {e}"),
            }
        }
        if need_freshwater {
            match download(&client, ISLEMAPS_FRESHWATER_URL, &freshwater_dest(), false) {
                Ok(_) => got_any = true,
                Err(e) => log::info!("islemaps freshwater top-up skipped: {e}"),
            }
        }
        if got_any {
            let _ = app.emit(
                "fetch://finished",
                FetchFinished {
                    ok: true,
                    basemap_ok: true,
                    pois_ok: true,
                    error: None,
                },
            );
        }
    });
}

fn convert() -> Result<(), String> {
    let read = |name: &str| {
        std::fs::read_to_string(settings::cache_dir().join(name)).map_err(|e| e.to_string())
    };
    let map_data = read("map-data.js")?;
    let ai_data = read("map-ai-spawn-zones.js")?;
    let water_txt = read("data_1.txt")?;

    let points = parse_point_pois(&map_data, &["saltrock", "mudwallow"]);
    let zones = parse_zones(&map_data, &["sanctuary", "migration", "patrol"]);
    let ai_zones = parse_ai_zones(&ai_data).unwrap_or_default();
    let water = parse_vulnona_text(&water_txt, "water");
    let regions = parse_vulnona_text(&water_txt, "area");
    let landmarks = parse_vulnona_text(&water_txt, "land");
    // Optional: the islemaps sighting chunk may be missing (fetch failed or
    // never ran) — then the animal layer is simply absent from the output.
    let animals = read("islemaps-sightings.js")
        .map(|js| parse_islemaps_animals(&js))
        .unwrap_or_default();

    let mut pois = json!({
        "version": POIS_VERSION,
        "map": MAP_VERSION,
        "units": "ue_cm",
        "_axis": "x = Lat (truc doc), y = Long (truc ngang)",
        "layers": {
            "water": { "kind": "point", "items": water },
            "saltlick": { "kind": "point", "items": points.get("saltrock").cloned().unwrap_or_default() },
            "mudwallow": { "kind": "point", "items": points.get("mudwallow").cloned().unwrap_or_default() },
            "sanctuary": { "kind": "zone", "items": zones.get("sanctuary").cloned().unwrap_or_default() },
            "migration": { "kind": "zone", "items": zones.get("migration").cloned().unwrap_or_default() },
            "patrol": { "kind": "zone", "items": zones.get("patrol").cloned().unwrap_or_default() },
            "food": { "kind": "zone", "items": ai_zones },
            "region": { "kind": "label", "items": regions },
            "landmark": { "kind": "label", "items": landmarks },
        },
    });
    if !animals.is_empty() {
        pois["layers"]["animal"] = json!({ "kind": "point", "items": animals });
    }
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
            // Optional alternative imagery, fetched only when selected in
            // Settings (basemap\islemaps\; freshness in its meta.json).
            "basemap_alt": {
                "islemaps": {
                    "urls": [
                        "https://www.islemaps.com/map/map-light.png",
                        "https://www.islemaps.com/map/map-dark.png",
                        "https://www.islemaps.com/layers/water.png",
                    ],
                    "credit": "IsleMaps.com (Pont & Emeara). Imagery (c) Afterthought LLC (The Isle).",
                },
            },
            "poi_sources": [
                { "layers": ["saltlick", "mudwallow", "sanctuary", "migration"],
                  "url": "https://myislemap.com/map-data.js", "credit": "myislemap.com" },
                { "layers": ["food"], "url": "https://myislemap.com/map-ai-spawn-zones.js",
                  "credit": "myislemap.com (datamined AI spawn zones)" },
                { "layers": ["water"], "url": format!("{base}/data_1.txt"),
                  "credit": "VulnonaMAP (Coco.N)" },
                { "layers": ["animal"], "url": "https://www.islemaps.com/ (map bundle)",
                  "credit": "IsleMaps.com (Pont & Emeara), community-collected AI spawn sightings" },
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
        let zones = parse_zones(&map_data, &["sanctuary", "migration", "patrol"]);
        let ai = parse_ai_zones(&ai_data).unwrap();

        assert_eq!(parse_vulnona_text(&water_txt, "water").len(), 27, "water");
        assert_eq!(parse_vulnona_text(&water_txt, "area").len(), 26, "region");
        assert_eq!(parse_vulnona_text(&water_txt, "land").len(), 48, "landmark");
        assert_eq!(points["saltrock"].len(), 24, "saltlick");
        assert_eq!(points["mudwallow"].len(), 36, "mudwallow");
        assert_eq!(zones["sanctuary"].len(), 7, "sanctuary");
        assert_eq!(zones["migration"].len(), 12, "migration");
        assert_eq!(zones["patrol"].len(), 61, "patrol");
        assert_eq!(ai.len(), 52, "food");
    }

    #[test]
    fn islemaps_animals_negate_lat_and_skip_salt() {
        let js = r#"const a=[{lat:390.427,lng:147.898,info:"Boar"},{lat:-268.3,lng:-85.84,info:"Deer"},
            {lat:1.0,lng:2.0,info:"Salt"},{lat:3.0,lng:4.0,info:"Fireweed"}];"#;
        let out = parse_islemaps_animals(js);
        assert_eq!(out.len(), 2, "Salt and plants are skipped");
        // Their lat = -gameX/1000: lat 390.427 -> gameX -390427 cm (north).
        assert_eq!(out[0]["label"], "Boar");
        assert!((out[0]["x"].as_f64().unwrap() - -390427.0).abs() < 0.5);
        assert!((out[0]["y"].as_f64().unwrap() - 147898.0).abs() < 0.5);
        assert!((out[1]["x"].as_f64().unwrap() - 268300.0).abs() < 0.5);
    }

    /// Minimal valid PNG prefix: signature + IHDR length/type + width/height.
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        b.extend_from_slice(&13u32.to_be_bytes());
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&width.to_be_bytes());
        b.extend_from_slice(&height.to_be_bytes());
        b
    }

    #[test]
    fn png_dimensions_reads_ihdr() {
        assert_eq!(png_dimensions(&png_header(2500, 2500)), Some((2500, 2500)));
        assert_eq!(png_dimensions(&png_header(1024, 768)), Some((1024, 768)));
    }

    #[test]
    fn png_dimensions_rejects_non_png_and_truncated() {
        assert_eq!(png_dimensions(b"RIFF....WEBP"), None, "webp is not png");
        assert_eq!(png_dimensions(&png_header(2500, 2500)[..20]), None, "truncated");
        assert_eq!(png_dimensions(b""), None);
        let mut wrong_chunk = png_header(2500, 2500);
        wrong_chunk[12..16].copy_from_slice(b"IDAT");
        assert_eq!(png_dimensions(&wrong_chunk), None, "first chunk must be IHDR");
    }

    #[test]
    fn water_parser_scales_thousands_to_cm() {
        let txt = "text\twater\tDam Lake\textra\n-267.0,79.0,\nother line\n";
        let water = parse_vulnona_text(txt, "water");
        assert_eq!(water[0]["label"], "Dam Lake");
        assert_eq!(water[0]["x"], -267000.0);
        assert_eq!(water[0]["y"], 79000.0);
    }

    #[test]
    fn text_parser_separates_kinds_and_skips_comments() {
        let txt = concat!(
            "text\tarea\tDelta\tlarge\n33,177,Delta,\n",
            "text\tland\tCentral Dome (Hexagon)\tlarge\n104,-43,Central Dome<s>(Hexagon)</s>,\n",
            "text\tland\t:Central Dome:comment\tcR small\n120,-41,[Now can't enter here],\n",
            "text\twater\tDam Lake\n-267.0,79.0,\n",
        );
        let regions = parse_vulnona_text(txt, "area");
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0]["label"], "Delta");
        assert_eq!(regions[0]["x"], 33000.0);

        let landmarks = parse_vulnona_text(txt, "land");
        // The clean name comes from column 3, not the marked-up display text,
        // and the ':'-prefixed comment record is dropped.
        assert_eq!(landmarks.len(), 1);
        assert_eq!(landmarks[0]["label"], "Central Dome (Hexagon)");

        assert_eq!(parse_vulnona_text(txt, "water").len(), 1);
    }
}
