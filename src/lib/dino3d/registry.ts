// Per-species 3D asset registry for the IslePilot skinviewer CDN.
//
// registry.json is extracted VERBATIM from the official overlay app's
// renderer bundle — the URLs are hand-maintained over there (abbreviated
// folder names, inconsistent filenames, typos included), so they cannot be
// derived from the species name. Species keys are exact-case.

import registryJson from "./registry.json";

export interface DinoModelEntry {
  name: string;
  folder: string;
  glbModel: string;
  normalMap: string;
  /** Adult pattern PNGs by pattern index (we always use 1, like the garage). */
  patterns: Record<string, string>;
  juvenilePattern?: string;
  racMap: string;
  /** Per-pattern teeth/mouth/claws mask (R/G/B channels). */
  patternMasks?: Record<string, string>;
  maskMap?: string;
  detailScale: number;
  glbScale: number;
  glbPosition: [number, number, number];
  /** Named idle animation to play (falls back to the first clip). */
  previewClip?: string;
}

export const DINO_MODELS = registryJson as unknown as Record<string, DinoModelEntry>;

export const hasModel = (species: string | null | undefined): boolean =>
  !!species && species in DINO_MODELS;

/** Shared textures (same for every species). */
export const SHARED = {
  detailNormal: "https://islepilot.eu/cdn/skinviewer/shared/T_DinoSkinDetail_5_N.png",
};

/** The 10 skin colour channels, hex strings. */
export type DinoPalette = {
  body: string;
  markings: string;
  flank: string;
  underbelly: string;
  detail: string;
  display: string;
  eyes: string;
  teeth: string;
  mouth: string;
  claws: string;
};

/** Neutral defaults — the official app's fallback palette. */
export const DEFAULT_PALETTE: DinoPalette = {
  body: "#6f8d44",
  markings: "#364725",
  flank: "#7b6a42",
  underbelly: "#b2b08e",
  detail: "#71815d",
  display: "#d5f38f",
  eyes: "#ffd76b",
  teeth: "#e8e2d0",
  mouth: "#7a3b3b",
  claws: "#3a3a3a",
};

/** Read a palette off a garage record defensively; missing/junk -> defaults. */
export function paletteFrom(raw: unknown): DinoPalette {
  const out = { ...DEFAULT_PALETTE };
  if (raw && typeof raw === "object") {
    for (const key of Object.keys(out) as (keyof DinoPalette)[]) {
      const v = (raw as Record<string, unknown>)[key];
      if (typeof v === "string" && /^#[0-9a-fA-F]{6}$/.test(v)) out[key] = v;
    }
  }
  return out;
}
