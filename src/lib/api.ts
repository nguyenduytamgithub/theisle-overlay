// The single typed IPC surface. Mirrors src-tauri/src/commands.rs and
// events.rs — if a shape changes there, it changes here.

import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---------------------------------------------------------------- events ---

export interface PositionUpdate {
  xCm: number;
  yCm: number;
  zCm: number;
  px: number;
  py: number;
  headingDeg: number | null;
  compassKey: string | null;
  inBounds: boolean;
}

export interface TrailPayload {
  segmentsCm: [number, number][][];
  segmentsPx: [number, number][][];
}

export type Settings = Record<string, unknown> & {
  minimap: {
    visible: boolean;
    require_game: boolean;
    corner: "top-left" | "top-right" | "bottom-left" | "bottom-right";
    size_px: number;
    margin_px: number;
    opacity: number;
    radius_m: number;
    click_through: boolean;
  };
  hotkeys: Record<string, string>;
  layers: Record<string, boolean>;
  map: { zone_labels: boolean };
  trail: {
    enabled: boolean;
    break_after_minutes: number;
    break_after_metres: number;
    min_node_distance_m: number;
  };
  number_format: "auto" | "us" | "eu";
  language: "vi" | "en";
  islepilot: {
    enabled: boolean;
    domain: string;
    poll_interval_s: number;
    use_map_position: boolean;
    show_overlay_panel: boolean;
  };
};

export const onPositionUpdate = (
  cb: (p: PositionUpdate) => void,
): Promise<UnlistenFn> => listen<PositionUpdate>("position://update", (e) => cb(e.payload));

export const onTrailChanged = (
  cb: (t: TrailPayload) => void,
): Promise<UnlistenFn> => listen<TrailPayload>("trail://changed", (e) => cb(e.payload));

export const onSettingsChanged = (
  cb: (s: Settings) => void,
): Promise<UnlistenFn> => listen<Settings>("settings://changed", (e) => cb(e.payload));

export const onWaypointsChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen("waypoints://changed", () => cb());

/**
 * Await-safe listener collection. The old pattern — pushing awaited unlisten
 * fns into an array the cleanup closes over — leaked every listener whose
 * `listen()` resolved after the component unmounted (fast tab switching).
 */
export function listenerBag() {
  let disposed = false;
  const fns: UnlistenFn[] = [];
  return {
    async add(p: Promise<UnlistenFn>): Promise<void> {
      const unlisten = await p;
      if (disposed) unlisten();
      else fns.push(unlisten);
    },
    dispose(): void {
      disposed = true;
      for (const fn of fns) fn();
      fns.length = 0;
    },
  };
}

export interface FailedHotkey {
  action: string;
  spec: string;
}

export const onHotkeyFailed = (
  cb: (failed: FailedHotkey[]) => void,
): Promise<UnlistenFn> => listen<FailedHotkey[]>("hotkey://failed", (e) => cb(e.payload));

// -------------------------------------------------------------- commands ---

export interface Waypoint {
  id: string;
  name: string;
  x: number;
  y: number;
  z: number;
  color: string | null;
  created: string | null;
}

export interface DataStatus {
  basemapMinimap: boolean;
  basemapFullmap: boolean;
  pois: boolean;
}

export const getSettings = () => invoke<Settings>("get_settings");
export const patchSettings = (patch: object) =>
  invoke<Settings>("patch_settings", { patch });

/** Last known position (null before the first sample) — for initial paint. */
export const getCurrentPosition = () =>
  invoke<PositionUpdate | null>("get_current_position");

export type WaypointPx = Waypoint & { px: number; py: number };

export const listWaypoints = () => invoke<Waypoint[]>("list_waypoints");
export const listWaypointsPx = () => invoke<WaypointPx[]>("list_waypoints_px");
export const addWaypointAtPixel = (px: number, py: number, name: string) =>
  invoke<Waypoint>("add_waypoint_at_pixel", { px, py, name });
export const addWaypointHere = (name: string) =>
  invoke<Waypoint | null>("add_waypoint_here", { name });
export const renameWaypoint = (id: string, name: string) =>
  invoke<boolean>("rename_waypoint", { id, name });
export const deleteWaypoint = (id: string) =>
  invoke<boolean>("delete_waypoint", { id });

export const getPreviousTrail = () => invoke<TrailPayload>("get_previous_trail");
export const getCurrentTrail = () => invoke<TrailPayload>("get_current_trail");

export const getDataStatus = () => invoke<DataStatus>("data_status");

/** Kick off the (re-)download; watch fetch:// events for progress/result. */
export const startFetchData = (force: boolean) => invoke("fetch_data", { force });

export interface FetchProgress {
  file: string;
  index: number;
  total: number;
  status: "downloading" | "done" | "skipped" | "error";
  error: string | null;
}

export interface FetchFinished {
  ok: boolean;
  basemapOk: boolean;
  poisOk: boolean;
  error: string | null;
}

export const onFetchProgress = (
  cb: (p: FetchProgress) => void,
): Promise<UnlistenFn> => listen<FetchProgress>("fetch://progress", (e) => cb(e.payload));

export const onFetchFinished = (
  cb: (f: FetchFinished) => void,
): Promise<UnlistenFn> => listen<FetchFinished>("fetch://finished", (e) => cb(e.payload));
export const getFullscreenMode = () => invoke<number | null>("get_fullscreen_mode");

/** POI layer data, shape produced by fetch_data (px precomputed at fetch). */
export const getPois = () => invoke<unknown>("get_pois");

export interface PoiItem {
  label: string;
  px: number;
  py: number;
  xCm: number;
  yCm: number;
  radiusPx?: number;
  pointsPx?: [number, number][];
  /** Zones: name-label anchor (polygon centroid / circle centre). */
  labelPx?: number;
  labelPy?: number;
}

export interface PoiLayer {
  key: string;
  kind: "point" | "zone" | "label";
  items: PoiItem[];
}

/** POI layers with all coordinates precomputed to basemap pixels by Rust. */
export const getPoisRender = () => invoke<PoiLayer[]>("get_pois_render");

export interface NearestWaypoint {
  id: string;
  name: string;
  bearingDeg: number;
  compassKey: string;
  distanceM: number;
}

export const getNearestWaypoint = () =>
  invoke<NearestWaypoint | null>("nearest_waypoint");

/** True when the spec parses AND the combination is currently free. */
export const checkHotkeyAvailable = (spec: string) =>
  invoke<boolean>("check_hotkey_available", { spec });

/** Re-register all hotkeys from the current settings (after a rebind). */
export const applyHotkeys = () => invoke("apply_hotkeys");

export async function getBasemapUrls(): Promise<{ minimap: string; fullmap: string }> {
  const paths = await invoke<{ minimap: string; fullmap: string }>("get_basemap_paths");
  return {
    minimap: convertFileSrc(paths.minimap),
    fullmap: convertFileSrc(paths.fullmap),
  };
}

// ----------------------------------------------------- "your dino" (IslePilot) ---

export interface DinoStatBar {
  raw: string;
  current: number | null;
  max: number | null;
}

export interface DinoQuest {
  text: string;
  completed: boolean;
}

export interface DinoPlayer {
  dinoName: string | null;
  online: boolean | null;
  growth: string | null;
  growthPct: number | null;
  health: DinoStatBar | null;
  hunger: DinoStatBar | null;
  thirst: DinoStatBar | null;
  primeQuests: DinoQuest[];
}

export interface DinoMap {
  mapDisabled: boolean;
  x: number | null;
  y: number | null;
  headingDeg: number | null;
  viewBox: [number, number, number, number] | null;
  pctX: number | null;
  pctY: number | null;
}

export interface DinoUpdate {
  domain: string;
  fetchedAtMs: number;
  player: DinoPlayer | null;
  map: DinoMap | null;
  layoutChanged: boolean;
  error: string | null;
}

export interface IslepilotState {
  loggedIn: boolean;
  lastUpdate: DinoUpdate | null;
}

export const islepilotLogin = (domain: string) =>
  invoke("islepilot_login", { domain });
/** Manual fallback: validate + store a pasted Cookie header. */
export const islepilotSetCookie = (domain: string, cookie: string) =>
  invoke("islepilot_set_cookie", { domain, cookie });
export const islepilotCancelLogin = () => invoke("islepilot_cancel_login");
export const islepilotLogout = () => invoke("islepilot_logout");
export const islepilotApply = () => invoke("islepilot_apply");
export const islepilotState = () => invoke<IslepilotState>("islepilot_state");

export const onDinoUpdate = (cb: (u: DinoUpdate) => void): Promise<UnlistenFn> =>
  listen<DinoUpdate>("dino://update", (e) => cb(e.payload));
export const onDinoAuthExpired = (cb: () => void): Promise<UnlistenFn> =>
  listen("dino://auth-expired", () => cb());
export const onDinoLoginOk = (cb: () => void): Promise<UnlistenFn> =>
  listen("dino://login-ok", () => cb());
export const onDinoLoginFailed = (
  cb: (reason: string) => void,
): Promise<UnlistenFn> => listen<string>("dino://login-failed", (e) => cb(e.payload));

/** Dev builds only. */
export const simulatePosition = (x: number, y: number, z: number) =>
  invoke("simulate_position", { x, y, z });
