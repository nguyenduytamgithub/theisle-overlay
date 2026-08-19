// Minimap overlay entry. Deliberately tiny: no Skeleton, no Leaflet, no
// framework — this webview runs beside the game for hours. Rendering is
// event-driven only (zero idle CPU: no rAF loop, no animations, no timers).

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { render, type MinimapState, type PoiDot } from "./render";

// Local minimal types — this bundle stays free of the main window's modules.
interface PositionUpdate {
  xCm: number;
  yCm: number;
  px: number;
  py: number;
  headingDeg: number | null;
  compassKey: string | null;
}
interface PoiLayer {
  key: string;
  kind: string;
  items: { label: string; px: number; py: number; xCm: number; yCm: number }[];
}
type Settings = Record<string, any>;

const LAYER_COLORS: Record<string, string> = {
  water: "#4aa8d8",
  saltlick: "#d9a441",
  mudwallow: "#9c7b4f",
  sanctuary: "#a855f7",
  migration: "#72d653",
  food: "#e2664a",
};

// Compass letters + strings per language (kept inline: no i18n bundle here).
const STRINGS = {
  vi: {
    letters: ["B", "Đ", "N", "T"] as [string, string, string, string],
    hint: "Trong game bấm Tab, rồi bấm “Asset Location” để chép tọa độ.",
    unknown: "Chưa rõ hướng",
    dirs: {
      "dir.N": "Bắc", "dir.NE": "Đông Bắc", "dir.E": "Đông", "dir.SE": "Đông Nam",
      "dir.S": "Nam", "dir.SW": "Tây Nam", "dir.W": "Tây", "dir.NW": "Tây Bắc",
    } as Record<string, string>,
  },
  en: {
    letters: ["N", "E", "S", "W"] as [string, string, string, string],
    hint: "In game press Tab, then click “Asset Location” to copy your coordinates.",
    unknown: "Heading unknown",
    dirs: {
      "dir.N": "N", "dir.NE": "NE", "dir.E": "E", "dir.SE": "SE",
      "dir.S": "S", "dir.SW": "SW", "dir.W": "W", "dir.NW": "NW",
    } as Record<string, string>,
  },
};

const canvas = document.getElementById("minimap") as HTMLCanvasElement;

let allPois: PoiDot[] = [];
let poiLayers: PoiLayer[] = [];
let settings: Settings = {};

const state: MinimapState = {
  position: null,
  trailPx: [],
  pois: [],
  basemap: null,
  miniScale: 1,
  pxPerM: 0.7,
  sizePx: 260,
  radiusM: 600,
  opacity: 0.85,
  compassLetters: STRINGS.vi.letters,
  hintText: STRINGS.vi.hint,
  headingLabel: "",
  headingUnknown: STRINGS.vi.unknown,
};

let lastHeadingKey: string | null = null;
let lastHeadingDeg: number | null = null;

function applySettings(s: Settings) {
  settings = s;
  const mm = s.minimap ?? {};
  state.sizePx = Number(mm.size_px ?? 260);
  state.radiusM = Number(mm.radius_m ?? 600);
  state.opacity = Number(mm.opacity ?? 0.85);
  const lang = (s.language === "en" ? "en" : "vi") as keyof typeof STRINGS;
  state.compassLetters = STRINGS[lang].letters;
  state.hintText = STRINGS[lang].hint;
  state.headingUnknown = STRINGS[lang].unknown;
  refreshHeadingLabel(lang);
  refreshPoiFilter();
}

function refreshHeadingLabel(lang: keyof typeof STRINGS) {
  state.headingLabel =
    lastHeadingKey && lastHeadingDeg !== null
      ? `${STRINGS[lang].dirs[lastHeadingKey] ?? ""} ${Math.round(lastHeadingDeg)}°`
      : "";
}

function refreshPoiFilter() {
  const visible = settings.layers ?? {};
  state.pois = allPois.filter((p) => visible[(p as any).layerKey] ?? true);
}

function flattenPois() {
  allPois = [];
  for (const layer of poiLayers) {
    if (layer.kind !== "point") continue; // zones are full-map only
    const color = LAYER_COLORS[layer.key] ?? "#e8a33d";
    for (const item of layer.items) {
      allPois.push({
        xCm: item.xCm,
        yCm: item.yCm,
        px: item.px,
        py: item.py,
        color,
        // carried for the visibility filter
        ...( { layerKey: layer.key } as object ),
      });
    }
  }
  refreshPoiFilter();
}

const draw = () => render(canvas, state);

async function init() {
  settings = await invoke<Settings>("get_settings");
  applySettings(settings);

  const info = await invoke<{ imageWidthPx: number; pxPerMX: number }>("get_map_info");
  state.pxPerM = info.pxPerMX;

  try {
    poiLayers = await invoke<PoiLayer[]>("get_pois_render");
    flattenPois();
  } catch {
    // POI data missing (first run): map still works without dots.
  }

  await listen<PositionUpdate>("position://update", (e) => {
    const p = e.payload;
    state.position = { xCm: p.xCm, yCm: p.yCm, px: p.px, py: p.py, headingDeg: p.headingDeg };
    lastHeadingKey = p.compassKey;
    lastHeadingDeg = p.headingDeg;
    refreshHeadingLabel(settings.language === "en" ? "en" : "vi");
    draw();
  });
  await listen<{ segmentsPx: [number, number][][] }>("trail://changed", (e) => {
    state.trailPx = e.payload.segmentsPx;
    draw();
  });
  await listen<Settings>("settings://changed", (e) => {
    applySettings(e.payload);
    draw();
  });

  // First paint before the window is shown (Rust shows it on this signal).
  draw();
  await emit("minimap://ready", {});

  // Basemap decode can lag behind the first paint; draw again when ready.
  try {
    const paths = await invoke<{ minimap: string }>("get_basemap_paths");
    const resp = await fetch(convertFileSrc(paths.minimap));
    if (resp.ok) {
      state.basemap = await createImageBitmap(await resp.blob());
      const infoFull = await invoke<{ imageWidthPx: number }>("get_map_info");
      state.miniScale = state.basemap.width / infoFull.imageWidthPx;
      draw();
    }
  } catch {
    // Missing basemap: the disc just stays unfilled.
  }
}

void init();
