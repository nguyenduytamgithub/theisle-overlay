//! Clipboard coordinate-string parsing. 1:1 port of the parsing half of
//! `app/coords.py`.
//!
//! The hard part: the comma is BOTH the thousands separator and the field
//! separator, so splitting on commas is wrong. Whole grouped number tokens
//! must be matched instead:
//!
//! ```text
//! "-231,654.353, 52,099.673, 29,328.085"   <- 3 numbers, not 6
//! ```

use std::sync::LazyLock;

use regex::Regex;

/// Largest accepted absolute value, in cm. Half the map width is ~610_000 cm;
/// this threshold blocks junk (codes, phone numbers, timestamps) while still
/// passing points beyond the charted edge like Hell's Mouth.
pub const MAX_ABS_CM: f64 = 2_000_000.0;

/// Longest clipboard text worth running the regexes on. Anything longer is the
/// user copying prose, not coordinates.
pub const MAX_CLIPBOARD_LEN: usize = 4096;

/// Below this (cm) the first two values cannot be a real position: 1000 cm is
/// only 10 m from the map-centre origin. Filters out "version 1.2.3".
pub const MIN_PLAUSIBLE_CM: f64 = 1000.0;

// US-style number: groups of 3 digits separated by commas, decimal point.
const NUM_US: &str = r"[-+]?(?:\d{1,3}(?:,\d{3})+|\d+)(?:\.\d+)?";
// EU-style number: the reverse.
const NUM_EU: &str = r"[-+]?(?:\d{1,3}(?:\.\d{3})+|\d+)(?:,\d+)?";

static RE_US: LazyLock<Regex> = LazyLock::new(|| Regex::new(NUM_US).unwrap());
static RE_EU: LazyLock<Regex> = LazyLock::new(|| Regex::new(NUM_EU).unwrap());

// Old legacy branch form: "(Lat: -123,456.789 Long: 123,456.789 Alt: 12.345)"
static RE_LEGACY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?is)lat\s*[:=]\s*({NUM_US}).*?long\s*[:=]\s*({NUM_US})(?:.*?alt\s*[:=]\s*({NUM_US}))?"
    ))
    .unwrap()
});

/// Number format the user's machine produces. Auto tries both and picks the
/// reading that yields the larger magnitudes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumberFormat {
    #[default]
    Auto,
    Us,
    Eu,
}

impl NumberFormat {
    /// Parse a settings string ("auto" / "us" / "eu"); unknown values fall
    /// back to Auto, mirroring the Python `.get(...)` default.
    pub fn from_setting(s: &str) -> Self {
        match s {
            "us" => Self::Us,
            "eu" => Self::Eu,
            _ => Self::Auto,
        }
    }
}

fn to_float_us(tok: &str) -> Option<f64> {
    tok.replace(',', "").parse().ok()
}

fn to_float_eu(tok: &str) -> Option<f64> {
    tok.replace('.', "").replace(',', ".").parse().ok()
}

/// Unicode minus signs and spaces that the game/browser may insert.
fn normalise(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{2212}' | '\u{2013}' | '\u{2014}' => '-', // MINUS SIGN, EN/EM DASH
            '\u{00A0}' | '\u{2009}' => ' ',              // NBSP, THIN SPACE
            c => c,
        })
        .collect()
}

/// Blocks clipboard content that is not a coordinate.
///
/// Three filter layers, each catching a different kind of junk:
///   1. Magnitudes must be within the map threshold  -> drops huge junk numbers.
///   2. At least one of the first two values must be large enough -> drops
///      "version 1.2.3".
///   3. At least one value must have a fractional part or be genuinely large
///      -> drops dates like "2026-08-19" (2026 is big-ish but a small integer).
fn plausible(vals: &[f64]) -> bool {
    if vals.len() < 2 {
        return false;
    }
    if vals.iter().any(|v| v.abs() > MAX_ABS_CM) {
        return false;
    }
    if vals[..2].iter().all(|v| v.abs() < MIN_PLAUSIBLE_CM) {
        return false;
    }
    vals[..2]
        .iter()
        .any(|v| v.fract() != 0.0 || v.abs() > 10_000.0)
}

fn extract(s: &str, rx: &Regex, conv: fn(&str) -> Option<f64>) -> Vec<f64> {
    let mut vals = Vec::with_capacity(3);
    for m in rx.find_iter(s) {
        if let Some(v) = conv(m.as_str()) {
            vals.push(v);
        }
        if vals.len() == 3 {
            break;
        }
    }
    vals
}

fn pad3(mut vals: Vec<f64>) -> (f64, f64, f64) {
    while vals.len() < 3 {
        vals.push(0.0);
    }
    (vals[0], vals[1], vals[2])
}

/// Extract (x, y, z) in cm from clipboard content.
///
/// Returns None when not recognised — the caller must SILENTLY ignore it: no
/// log, no UI flash, so the user's normal copy/paste feels untouched.
pub fn parse_coordinates(text: &str, format: NumberFormat) -> Option<(f64, f64, f64)> {
    let s = text.trim();
    if s.is_empty() || s.chars().count() > MAX_CLIPBOARD_LEN {
        return None;
    }
    let s = normalise(s);

    // The legacy form carries its own anchoring keywords; try it first because
    // it is the least ambiguous.
    if let Some(caps) = RE_LEGACY.captures(&s) {
        let vals: Vec<f64> = caps
            .iter()
            .skip(1)
            .flatten()
            .filter_map(|g| to_float_us(g.as_str()))
            .collect();
        if plausible(&vals) {
            return Some(pad3(vals));
        }
    }

    type Reader = (&'static Regex, fn(&str) -> Option<f64>);
    let readers: &[Reader] = match format {
        NumberFormat::Us => &[(&RE_US, to_float_us)],
        NumberFormat::Eu => &[(&RE_EU, to_float_eu)],
        NumberFormat::Auto => &[(&RE_US, to_float_us), (&RE_EU, to_float_eu)],
    };

    // When both readings pass the filters, take the one with larger values
    // (first wins on a tie, like Python's max()): the correct reader keeps
    // "231.654,353" as ONE number; the wrong one shreds it into 231.654 and
    // 353, thousands of times smaller.
    let mut best: Option<Vec<f64>> = None;
    for (rx, conv) in readers {
        let vals = extract(&s, rx, *conv);
        if !plausible(&vals) {
            continue;
        }
        let better = match &best {
            Some(b) => vals[0].abs() + vals[1].abs() > b[0].abs() + b[1].abs(),
            None => true,
        };
        if better {
            best = Some(vals);
        }
    }
    best.map(pad3)
}
