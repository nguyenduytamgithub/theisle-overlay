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

// Keys match pois_gateway.json layer keys.
export const LAYER_COLORS: Record<string, string> = {
  water: "#4aa8d8",
  saltlick: "#d9a441",
  mudwallow: "#9c7b4f",
  sanctuary: "#a855f7",
  migration: "#72d653",
  food: "#e2664a",
  patrol: "#ef6f6c", // myislemap's original patrol colour
  region: "#eae6d6",
  landmark: "#cfc9b3",
};

// Draw order: big zones first, small dots after, text labels on top.
export const LAYER_ORDER = [
  "patrol",
  "migration",
  "sanctuary",
  "food",
  "water",
  "mudwallow",
  "saltlick",
  "landmark",
  "region",
];

export const ZONE_FILL_OPACITY = 60 / 255;
export const ZONE_STROKE_OPACITY = 190 / 255;

export const POI_DOT_RADIUS = 5;
export const PLAYER_DOT_RADIUS = 7;
export const WAYPOINT_RADIUS = 6;

// Original basemap space; the Leaflet scene uses these bounds.
export const BASEMAP_W = 7800;
export const BASEMAP_H = 7817;
