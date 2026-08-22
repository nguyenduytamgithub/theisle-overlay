// Skin-texture compositing for the 3D dino viewer — a faithful port of the
// official overlay app's CPU pipeline (no shaders): the pattern PNG encodes
// each colour zone as a reference colour, and we replace nearest-matching
// pixels with the dino's palette, then blend the teeth/mouth/claws mask, the
// RAC cavity map and a detail normal. All plain ImageData math on canvases.

import { fetchCdnAsset } from "$lib/api";
import { SHARED, type DinoModelEntry, type DinoPalette } from "./registry";

/** Reference colours baked into the pattern PNGs (0-255 RGB). */
const ZONE_REFS: { key: keyof DinoPalette; ref: [number, number, number]; threshold: number }[] = [
  { key: "display", ref: [255, 0, 0], threshold: 0.42 },
  { key: "underbelly", ref: [0, 255, 0], threshold: 0.42 },
  { key: "flank", ref: [0, 1, 245], threshold: 0.42 },
  { key: "body", ref: [0, 255, 241], threshold: 0.42 },
  { key: "markings", ref: [255, 0, 255], threshold: 0.6 },
  { key: "detail", ref: [255, 255, 0], threshold: 0.42 },
];

/** Same brightness factor the official app multiplies replacements by. */
const BRIGHTNESS = 0.55;
/** RAC cavity strength (uses G*B of the RAC map). */
const RAC_STRENGTH = 0.85;

const hexToRgb = (hex: string): [number, number, number] => {
  const n = parseInt(hex.replace("#", ""), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
};

/// Per-pixel JS compositing on a 4K texture is seconds of main-thread work
/// for no visible gain in a preview widget — cap everything at 2K.
const MAX_TEX = 2048;

/// Decoded source textures by URL — the shared detail normal is reused by
/// every species, and a palette change only needs the composite redone.
/// NOTE: composite passes must never mutate these (they copy first).
const imageCache = new Map<string, Promise<ImageData>>();

function loadImageData(url: string): Promise<ImageData> {
  let p = imageCache.get(url);
  if (!p) {
    p = decodeImage(url);
    p.catch(() => imageCache.delete(url));
    imageCache.set(url, p);
  }
  return p;
}

async function decodeImage(url: string): Promise<ImageData> {
  const buf = await fetchCdnAsset(url);
  const bitmap = await createImageBitmap(new Blob([buf]));
  const scale = Math.min(1, MAX_TEX / Math.max(bitmap.width, bitmap.height));
  const w = Math.max(1, Math.round(bitmap.width * scale));
  const h = Math.max(1, Math.round(bitmap.height * scale));
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
  ctx.drawImage(bitmap, 0, 0, w, h);
  bitmap.close();
  return ctx.getImageData(0, 0, w, h);
}

function toCanvas(img: ImageData): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = img.width;
  canvas.height = img.height;
  canvas.getContext("2d")!.putImageData(img, 0, 0);
  return canvas;
}

/** Sample an ImageData with wrap-around (for the tiled detail normal). */
const sampleWrapped = (img: ImageData, x: number, y: number): number => {
  const xi = ((x % img.width) + img.width) % img.width;
  const yi = ((y % img.height) + img.height) % img.height;
  return (yi * img.width + xi) * 4;
};

/**
 * Build the recoloured base-colour canvas: zone replacement -> TMC mask ->
 * RAC cavity darkening.
 */
function compositeMap(
  pattern: ImageData,
  palette: DinoPalette,
  tmc: ImageData | null,
  rac: ImageData | null,
): HTMLCanvasElement {
  const zones = ZONE_REFS.map((z) => ({
    ...z,
    color: hexToRgb(palette[z.key]).map((c) => c * BRIGHTNESS) as unknown as [
      number,
      number,
      number,
    ],
  }));
  const teeth = hexToRgb(palette.teeth);
  const mouth = hexToRgb(palette.mouth);
  const claws = hexToRgb(palette.claws);

  // Work on a COPY — the source ImageData lives in the shared decode cache.
  pattern = new ImageData(
    new Uint8ClampedArray(pattern.data),
    pattern.width,
    pattern.height,
  );
  const d = pattern.data;
  const scaleTmc = tmc && (tmc.width !== pattern.width || tmc.height !== pattern.height);
  const scaleRac = rac && (rac.width !== pattern.width || rac.height !== pattern.height);
  for (let i = 0; i < d.length; i += 4) {
    let r = d[i], g = d[i + 1], b = d[i + 2];

    // 1. Nearest-reference-colour replacement (distances in 0..1 space).
    let best = -1;
    let bestDist = Infinity;
    for (let z = 0; z < zones.length; z++) {
      const ref = zones[z].ref;
      const dr = (r - ref[0]) / 255;
      const dg = (g - ref[1]) / 255;
      const db = (b - ref[2]) / 255;
      const dist = dr * dr + dg * dg + db * db;
      if (dist < bestDist) {
        bestDist = dist;
        best = z;
      }
    }
    if (best >= 0 && bestDist <= zones[best].threshold) {
      [r, g, b] = zones[best].color;
    }

    const px = (i / 4) % pattern.width;
    const py = Math.floor(i / 4 / pattern.width);

    // 2. Teeth/mouth/claws mask: R/G/B channels alpha-blend their colours.
    if (tmc) {
      const j = scaleTmc
        ? sampleWrapped(
            tmc,
            Math.floor((px / pattern.width) * tmc.width),
            Math.floor((py / pattern.height) * tmc.height),
          )
        : i;
      const tR = tmc.data[j] / 255;
      const tG = tmc.data[j + 1] / 255;
      const tB = tmc.data[j + 2] / 255;
      if (tR > 0) { r = r * (1 - tR) + teeth[0] * tR; g = g * (1 - tR) + teeth[1] * tR; b = b * (1 - tR) + teeth[2] * tR; }
      if (tG > 0) { r = r * (1 - tG) + mouth[0] * tG; g = g * (1 - tG) + mouth[1] * tG; b = b * (1 - tG) + mouth[2] * tG; }
      if (tB > 0) { r = r * (1 - tB) + claws[0] * tB; g = g * (1 - tB) + claws[1] * tB; b = b * (1 - tB) + claws[2] * tB; }
    }

    // 3. RAC cavity darkening: f = 1 - k*(1 - G*B).
    if (rac) {
      const j = scaleRac
        ? sampleWrapped(
            rac,
            Math.floor((px / pattern.width) * rac.width),
            Math.floor((py / pattern.height) * rac.height),
          )
        : i;
      const f = 1 - RAC_STRENGTH * (1 - (rac.data[j + 1] / 255) * (rac.data[j + 2] / 255));
      r *= f; g *= f; b *= f;
    }

    d[i] = r; d[i + 1] = g; d[i + 2] = b;
    // Fully opaque: any alpha baked into the pattern PNG must not darken the
    // texture through canvas premultiplication.
    d[i + 3] = 255;
  }
  return toCanvas(pattern);
}

/** Blend the species normal map with the shared detail normal (tangent add). */
function compositeNormal(
  base: ImageData,
  detail: ImageData,
  detailScale: number,
): HTMLCanvasElement {
  // Work on a COPY — the source ImageData lives in the shared decode cache.
  base = new ImageData(new Uint8ClampedArray(base.data), base.width, base.height);
  const d = base.data;
  for (let i = 0; i < d.length; i += 4) {
    const px = (i / 4) % base.width;
    const py = Math.floor(i / 4 / base.width);
    const j = sampleWrapped(
      detail,
      Math.floor((px / base.width) * detail.width * detailScale),
      Math.floor((py / base.height) * detail.height * detailScale),
    );
    const bx = (d[i] / 255) * 2 - 1;
    const by = (d[i + 1] / 255) * 2 - 1;
    const bz = (d[i + 2] / 255) * 2 - 1;
    const dx = (detail.data[j] / 255) * 2 - 1;
    const dy = (detail.data[j + 1] / 255) * 2 - 1;
    let nx = bx + dx;
    let ny = by + dy;
    let nz = Math.max(bz, 0.01);
    const len = Math.sqrt(nx * nx + ny * ny + nz * nz) || 1;
    nx /= len; ny /= len; nz /= len;
    d[i] = ((nx + 1) / 2) * 255;
    d[i + 1] = ((ny + 1) / 2) * 255;
    d[i + 2] = ((nz + 1) / 2) * 255;
  }
  return toCanvas(base);
}

export interface SkinCanvases {
  map: HTMLCanvasElement;
  normal: HTMLCanvasElement | null;
}

/** Composited skins by species+palette — a tab switch or re-selection of
 * the same dino re-uses the finished canvases outright. */
const skinCache = new Map<string, Promise<SkinCanvases>>();
const SKIN_CACHE_MAX = 12;

/** Stable cache key for a species + palette combination. */
export const skinKey = (species: string, palette: DinoPalette): string =>
  `${species}|${Object.values(palette).join(",")}`;

/** Build the recoloured skin for one species + palette (pattern index 1). */
export function buildSkin(
  entry: DinoModelEntry,
  palette: DinoPalette,
): Promise<SkinCanvases> {
  const key = skinKey(entry.name, palette);
  let p = skinCache.get(key);
  if (!p) {
    p = buildSkinUncached(entry, palette);
    p.catch(() => skinCache.delete(key));
    if (skinCache.size >= SKIN_CACHE_MAX) {
      const oldest = skinCache.keys().next().value;
      if (oldest !== undefined) skinCache.delete(oldest);
    }
    skinCache.set(key, p);
  }
  return p;
}

async function buildSkinUncached(
  entry: DinoModelEntry,
  palette: DinoPalette,
): Promise<SkinCanvases> {
  const patternUrl = entry.patterns["1"] ?? Object.values(entry.patterns)[0];
  const tmcUrl = entry.patternMasks?.["1"] ?? null;
  const [pattern, tmc, rac, normal, detailNormal] = await Promise.all([
    loadImageData(patternUrl),
    tmcUrl ? loadImageData(tmcUrl).catch(() => null) : Promise.resolve(null),
    loadImageData(entry.racMap).catch(() => null),
    loadImageData(entry.normalMap).catch(() => null),
    loadImageData(SHARED.detailNormal).catch(() => null),
  ]);
  return {
    map: compositeMap(pattern, palette, tmc, rac),
    normal:
      normal && detailNormal
        ? compositeNormal(normal, detailNormal, entry.detailScale || 12)
        : normal
          ? toCanvas(normal)
          : null,
  };
}
