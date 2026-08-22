// Colours and sizes carried over from the original theme.py — survival-HUD
// dark ground with an amber accent. No display strings here (see i18n/).

export const COLORS = {
  bg: "#11150e",
  panel: "#191f14",
  panelBorder: "#333c26",
  text: "#eae6d6",
  textMuted: "#a3aa8c",
  accent: "#e8a33d",
  player: "#ff3b8b", // pink: collides with no terrain colour
  // Electric yellow + double outline: the self-marker must outrank every
  // waypoint/POI dot and never be mistaken for the (softer yellow) trail.
  playerArrow: "#ffe600",
  playerArrowOutline: "#10130c",
  trail: "#ffcc55",
  waypoint: "#4fc3f7",
} as const;

// Keys match pois_gateway.json layer keys (+ image-overlay layer keys).
export const LAYER_COLORS: Record<string, string> = {
  freshwater: "#149af2", // islemaps.com's own fresh-water blue
  water: "#4aa8d8",
  saltlick: "#d9a441",
  mudwallow: "#9c7b4f",
  sanctuary: "#a855f7",
  migration: "#72d653",
  food: "#e2664a",
  patrol: "#ef6f6c", // myislemap's original patrol colour
  animal: "#d66ba0", // islemaps.com AI spawn sightings
  region: "#eae6d6",
  landmark: "#cfc9b3",
  islepilot: "#34d399", // live server POIs from the IslePilot overlay API
};

// Draw order: image overlays lowest, big zones next, small dots after, text
// labels on top.
export const LAYER_ORDER = [
  "freshwater",
  "islepilot",
  "patrol",
  "migration",
  "sanctuary",
  "food",
  "water",
  "mudwallow",
  "saltlick",
  "animal",
  "landmark",
  "region",
];

// Waypoint icon presets (offered in the naming prompt). A waypoint whose
// name STARTS with one of these renders as that glyph on both maps instead
// of a colour dot — the name itself is the single source of truth, so the
// on-disk waypoint format stays byte-compatible.
export const WAYPOINT_GLYPHS = ["💀", "🏠", "💧", "⚠️", "🍖"];

/** The glyph a waypoint renders as, or undefined for the plain colour dot. */
export function waypointGlyph(name: string): string | undefined {
  return WAYPOINT_GLYPHS.find((g) => name.startsWith(g));
}

// One recognisable glyph per animal species (labels from the islemaps
// sighting data). Rendered as text: Segoe UI Emoji covers all of these on
// Windows. Species without a glyph fall back to the layer-colour dot.
export const ANIMAL_GLYPHS: Record<string, string> = {
  Boar: "🐗",
  Bunny: "🐰",
  Chicken: "🐔",
  Crab: "🦀",
  Deer: "🦌",
  Frog: "🐸",
  Goat: "🐐",
  Teno: "🦕",
  Turtle: "🐢",
};

export const ZONE_FILL_OPACITY = 60 / 255;
export const ZONE_STROKE_OPACITY = 190 / 255;

export const POI_DOT_RADIUS = 5;
export const PLAYER_DOT_RADIUS = 7;
export const WAYPOINT_RADIUS = 6;

// Basemap geometry deliberately lives in Rust (get_map_info) — it varies with
// the selected basemap source, so no pixel constants belong here.
