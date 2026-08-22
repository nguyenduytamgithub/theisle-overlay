// Session-wide caches for the 3D viewer. Parsing a 3-10 MB GLB and
// compositing a 2K skin are both expensive — do each ONCE per species (/
// palette) and hand out clones, so tab switches and re-selections are free.

import { fetchCdnAsset, islepilotCdnAsset } from "$lib/api";
import { DINO_MODELS, SHARED, type DinoModelEntry } from "./registry";

type GLTF = import("three/examples/jsm/loaders/GLTFLoader.js").GLTF;

const gltfCache = new Map<string, Promise<GLTF>>();

/** Parsed GLB per species — cached as a promise so concurrent viewers share
 * one download+parse. The cached scene is a TEMPLATE: always clone it with
 * SkeletonUtils before adding to a scene. */
export function loadGltf(entry: DinoModelEntry): Promise<GLTF> {
  let p = gltfCache.get(entry.name);
  if (!p) {
    p = (async () => {
      const [{ GLTFLoader }, buf] = await Promise.all([
        import("three/examples/jsm/loaders/GLTFLoader.js"),
        fetchCdnAsset(entry.glbModel),
      ]);
      return new GLTFLoader().parseAsync(buf, "");
    })();
    // A failed download must not poison the cache forever.
    p.catch(() => gltfCache.delete(entry.name));
    gltfCache.set(entry.name, p);
  }
  return p;
}

const prefetched = new Set<string>();

/**
 * Warm the Rust disk cache for the given species in the background — one
 * asset at a time so the download never competes with a viewer the user is
 * actually looking at. Selecting a prefetched dino then opens instantly.
 */
export async function prefetchSpeciesAssets(speciesList: (string | null)[]): Promise<void> {
  for (const sp of speciesList) {
    if (!sp || prefetched.has(sp)) continue;
    prefetched.add(sp);
    const entry = DINO_MODELS[sp];
    if (!entry) continue;
    const urls = [
      entry.glbModel,
      entry.patterns["1"] ?? Object.values(entry.patterns)[0],
      entry.patternMasks?.["1"],
      entry.racMap,
      entry.normalMap,
      SHARED.detailNormal,
    ].filter((u): u is string => !!u);
    for (const url of urls) {
      try {
        await islepilotCdnAsset(url);
      } catch {
        // Offline / CDN hiccup: the viewer surfaces errors when it matters.
        prefetched.delete(sp);
        return;
      }
    }
  }
}
