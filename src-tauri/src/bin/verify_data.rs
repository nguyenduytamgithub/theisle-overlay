//! Check that the POI data matches the basemap image. Port of
//! `tools/verify_data.py`.
//!
//!     cargo run --bin verify_data --features devtools [-- --source <src>]
//!     <src>: vulnona (default) | islemaps-light | islemaps-dark
//!
//! Why: the arithmetic tests only verify the FORMULA. They cannot catch "the
//! right formula paired with the wrong basemap version". The check: sample
//! the basemap pixel colour under every POI — salt licks, mud wallows and
//! lakeshores must sit on LAND. If too many land in ocean, or swapping the
//! axes scores BETTER, something is wrong. Run after every map update, and
//! for every source after touching a calibration.

use overlay_core::{world_to_pixel, MapSource};
use theisle_overlay_lib::settings;

const STRICT_MAX_PCT: f64 = 2.0;

/// Layers that must sit on land, and the overall ocean-rate cap — per source.
/// The dark islemaps style reuses the SEA fill for lakes and rivers, so
/// water-adjacent layers (lake name labels ON the water, sanctuary polygons
/// crossing rivers) legitimately sample as "ocean" there; the mis-transform
/// signal on dark is the pure-land layers plus the axis-swap comparison.
fn strict_layers(source: MapSource) -> &'static [&'static str] {
    match source {
        MapSource::IslemapsDark => &["saltlick", "mudwallow"],
        _ => &["water", "saltlick", "mudwallow", "sanctuary"],
    }
}

fn overall_max_pct(source: MapSource) -> f64 {
    match source {
        MapSource::IslemapsDark => 15.0,
        _ => 10.0,
    }
}

fn parse_source() -> Result<MapSource, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().position(|a| a == "--source") {
        None => Ok(MapSource::Vulnona),
        Some(i) => {
            let value = args.get(i + 1).ok_or("--source needs a value")?;
            // Accept both spellings; the settings key uses '_'.
            MapSource::try_from_key(&value.replace('-', "_"))
                .ok_or_else(|| format!("unknown source {value:?} (vulnona | islemaps-light | islemaps-dark)"))
        }
    }
}

fn main() -> std::process::ExitCode {
    // --reconvert: rebuild pois_gateway.json from the cached source files
    // first (offline), so a POIS_VERSION bump or a freshly cached source can
    // be verified without launching the app.
    if std::env::args().any(|a| a == "--reconvert") {
        theisle_overlay_lib::fetch::ensure_pois_current();
    }
    let source = match parse_source() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let cal = source.calibration();
    let img_path = match theisle_overlay_lib::fetch::IslemapsVariant::for_source(source) {
        Some(variant) => variant.dest(),
        None => settings::basemap_dir().join("fullmap.webp"),
    };
    println!("source: {}  image: {}", source.key(), img_path.display());
    let img = match image::open(&img_path) {
        Ok(i) => i.to_rgb8(),
        Err(e) => {
            eprintln!("cannot open basemap {}: {e}", img_path.display());
            return std::process::ExitCode::FAILURE;
        }
    };
    let pois: serde_json::Value = match std::fs::read_to_string(settings::pois_path())
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("cannot read pois: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let sx = img.width() as f64 / cal.image_width_px as f64;
    let sy = img.height() as f64 / cal.image_height_px as f64;

    // Ocean test per imagery family. Vulnona's ocean is photographic navy
    // (relative channel test); islemaps paints the sea as ONE flat fill, so a
    // near-exact match against the sampled fill colour is the sharp test.
    let ocean_rgb = |r: i32, g: i32, b: i32| -> bool {
        let near = |v: i32, want: i32| (v - want).abs() <= 8;
        match source {
            MapSource::Vulnona => b > r + 25 && b > g + 18 && r < 90,
            MapSource::IslemapsLight => near(r, 48) && near(g, 56) && near(b, 72),
            MapSource::IslemapsDark => near(r, 17) && near(g, 24) && near(b, 26),
        }
    };
    let is_ocean = |px: f64, py: f64| -> Option<bool> {
        let ix = (px * sx) as i64;
        let iy = (py * sy) as i64;
        if ix < 0 || iy < 0 || ix >= img.width() as i64 || iy >= img.height() as i64 {
            return None;
        }
        let p = img.get_pixel(ix as u32, iy as u32);
        Some(ocean_rgb(p[0] as i32, p[1] as i32, p[2] as i32))
    };

    let survey = |swap: bool| -> Vec<(String, u64, u64)> {
        let mut out = Vec::new();
        let Some(layers) = pois.get("layers").and_then(|l| l.as_object()) else {
            return out;
        };
        for (name, layer) in layers {
            let (mut ocean, mut total) = (0u64, 0u64);
            for it in layer.get("items").and_then(|i| i.as_array()).unwrap_or(&Vec::new()) {
                let pts: Vec<(f64, f64)> = match it.get("points").and_then(|p| p.as_array()) {
                    Some(points) if !points.is_empty() => points
                        .iter()
                        .filter_map(|p| Some((p.get(0)?.as_f64()?, p.get(1)?.as_f64()?)))
                        .collect(),
                    _ => match (it.get("x").and_then(|v| v.as_f64()), it.get("y").and_then(|v| v.as_f64())) {
                        (Some(x), Some(y)) => vec![(x, y)],
                        _ => Vec::new(),
                    },
                };
                for (x, y) in pts {
                    let (x, y) = if swap { (y, x) } else { (x, y) };
                    let (px, py) = world_to_pixel(x, y, cal);
                    if let Some(res) = is_ocean(px, py) {
                        total += 1;
                        ocean += res as u64;
                    }
                }
            }
            if total > 0 {
                out.push((name.clone(), ocean, total));
            }
        }
        out
    };

    let good = survey(false);
    let bad = survey(true);

    println!("POI-in-ocean rates (low is good):\n");
    let mut failures = Vec::new();
    let (mut g_ocean, mut g_total) = (0u64, 0u64);
    for (name, o, n) in &good {
        let pct = 100.0 * *o as f64 / *n as f64;
        g_ocean += o;
        g_total += n;
        let mut mark = "";
        if strict_layers(source).contains(&name.as_str()) && pct > STRICT_MAX_PCT {
            mark = "  <-- TOO HIGH";
            failures.push(format!("{name}: {pct:.1}% > {STRICT_MAX_PCT}%"));
        }
        println!("  {name:<11} {o:>3}/{n:<4} = {pct:5.1}%{mark}");
    }

    let overall = if g_total > 0 { 100.0 * g_ocean as f64 / g_total as f64 } else { 0.0 };
    let b_ocean: u64 = bad.iter().map(|(_, o, _)| o).sum();
    let b_total: u64 = bad.iter().map(|(_, _, n)| n).sum();
    let swapped = if b_total > 0 { 100.0 * b_ocean as f64 / b_total as f64 } else { 0.0 };

    println!("\n  overall            {overall:5.1}%");
    println!("  with axes swapped  {swapped:5.1}%   (must be CLEARLY worse)");

    let overall_cap = overall_max_pct(source);
    if overall > overall_cap {
        failures.push(format!("overall {overall:.1}% > {overall_cap}%"));
    }
    if swapped <= overall {
        failures.push("axis swap scores equal or better — axes may be wrong".to_string());
    }

    println!();
    if !failures.is_empty() {
        println!("FAIL:");
        for f in &failures {
            println!("  - {f}");
        }
        return std::process::ExitCode::FAILURE;
    }
    println!("PASS. Transform and basemap agree.");
    std::process::ExitCode::SUCCESS
}
