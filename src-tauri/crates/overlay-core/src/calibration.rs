//! Calibration constants for one map version. Port of the `Calibration` half of
//! `app/coords.py`; the JSON file is copied verbatim from the original project
//! and embedded at compile time so the transform can never run against a
//! missing or stale file.

use std::sync::LazyLock;

use serde::Deserialize;

/// The calibration file, embedded verbatim. Tests read the selftest anchors
/// out of this same string so code and fixtures cannot drift apart.
pub const CALIBRATION_JSON: &str = include_str!("../data/calibration.json");

/// Calibration for the islemaps.com imagery (light and dark share one
/// geometry — same 2500x2500 frame, same world extent).
pub const CALIBRATION_ISLEMAPS_JSON: &str = include_str!("../data/calibration_islemaps.json");

/// Transform constants for one map version.
///
/// Verified against VulnonaMAP `js/map.js` ($map.calc.game2map) and
/// independently against `myislemap.com/app.js` gameToMap() — two authors,
/// identical constants. Do not re-derive; see `coords.rs` for the axis traps.
#[derive(Debug, Clone, PartialEq)]
pub struct Calibration {
    pub map_name: String,
    /// Game X range (km-ish units, cm/1000) — maps to the VERTICAL image axis.
    pub min_x: f64,
    pub max_x: f64,
    /// Game Y range — maps to the HORIZONTAL image axis.
    pub min_y: f64,
    pub max_y: f64,
    pub image_width_px: u32,
    pub image_height_px: u32,
    pub north_offset_deg: f64,
}

impl Calibration {
    /// Width of the game-X value range — mapped onto the image HEIGHT.
    pub fn span_x(&self) -> f64 {
        self.max_x - self.min_x
    }

    /// Width of the game-Y value range — mapped onto the image WIDTH.
    pub fn span_y(&self) -> f64 {
        self.max_y - self.min_y
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let raw: RawCalibration = serde_json::from_str(json)?;
        Ok(Self {
            map_name: raw.map,
            min_x: raw.min_x,
            max_x: raw.max_x,
            min_y: raw.min_y,
            max_y: raw.max_y,
            image_width_px: raw.image_width_px,
            image_height_px: raw.image_height_px,
            north_offset_deg: raw.north_offset_deg,
        })
    }

    /// The embedded Gateway calibration.
    pub fn gateway() -> &'static Calibration {
        static GATEWAY: LazyLock<Calibration> = LazyLock::new(|| {
            Calibration::from_json(CALIBRATION_JSON)
                .expect("embedded calibration.json must parse")
        });
        &GATEWAY
    }

    /// The embedded islemaps.com calibration (shared by light and dark).
    pub fn islemaps() -> &'static Calibration {
        static ISLEMAPS: LazyLock<Calibration> = LazyLock::new(|| {
            Calibration::from_json(CALIBRATION_ISLEMAPS_JSON)
                .expect("embedded calibration_islemaps.json must parse")
        });
        &ISLEMAPS
    }
}

/// Which basemap imagery the user selected. One entry per *calibration frame*
/// x style; a future multi-map feature adds a map dimension, not more variants
/// here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapSource {
    #[default]
    Vulnona,
    IslemapsLight,
    IslemapsDark,
}

impl MapSource {
    /// Lenient: unknown/missing settings values fall back to Vulnona (the
    /// imagery that is guaranteed to exist after first run).
    pub fn from_key(key: &str) -> MapSource {
        Self::try_from_key(key).unwrap_or_default()
    }

    /// Strict: for validating IPC input. None on unknown key.
    pub fn try_from_key(key: &str) -> Option<MapSource> {
        match key {
            "vulnona" => Some(MapSource::Vulnona),
            "islemaps_light" => Some(MapSource::IslemapsLight),
            "islemaps_dark" => Some(MapSource::IslemapsDark),
            _ => None,
        }
    }

    /// The settings/IPC key for this source.
    pub fn key(self) -> &'static str {
        match self {
            MapSource::Vulnona => "vulnona",
            MapSource::IslemapsLight => "islemaps_light",
            MapSource::IslemapsDark => "islemaps_dark",
        }
    }

    /// The calibration frame this imagery is drawn in. Light and dark share
    /// one geometry, so they return the identical `&'static`.
    pub fn calibration(self) -> &'static Calibration {
        match self {
            MapSource::Vulnona => Calibration::gateway(),
            MapSource::IslemapsLight | MapSource::IslemapsDark => Calibration::islemaps(),
        }
    }
}

#[derive(Deserialize)]
struct RawCalibration {
    map: String,
    #[serde(rename = "min_X")]
    min_x: f64,
    #[serde(rename = "max_X")]
    max_x: f64,
    #[serde(rename = "min_Y")]
    min_y: f64,
    #[serde(rename = "max_Y")]
    max_y: f64,
    image_width_px: u32,
    image_height_px: u32,
    #[serde(default)]
    north_offset_deg: f64,
}

// ---------------------------------------------------------------------------
// Selftest fixtures (golden anchors) — used by the test suite, read from the
// same embedded JSON as the constants themselves.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Anchor {
    pub name: String,
    /// [game X cm, game Y cm]
    pub raw: [f64; 2],
    pub px: f64,
    pub py: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SelfTest {
    #[serde(rename = "selftest_tolerance_px")]
    pub tolerance_px: f64,
    #[serde(rename = "selftest_anchors")]
    pub anchors: Vec<Anchor>,
    #[serde(rename = "selftest_out_of_bounds")]
    pub out_of_bounds: Anchor,
}

impl SelfTest {
    pub fn embedded() -> Self {
        Self::from_json(CALIBRATION_JSON)
    }

    /// Selftest block of any embedded calibration file (panics on a fixture
    /// that does not parse — these are compile-time constants, not user data).
    pub fn from_json(json: &str) -> Self {
        serde_json::from_str(json).expect("embedded calibration selftest block must parse")
    }
}
