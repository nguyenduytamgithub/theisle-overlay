import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { error } from "@tauri-apps/plugin-log";

import { installGlobalErrorLog } from "../lib/errlog";
import {
  bearingTo,
  compassPoint,
  distanceMetres,
  relativeBearing,
  type HudLanguage,
} from "../lib/navigation/guidance";
import {
  projectedPosition,
  smoothedPosition,
  type ProjectedPosition,
} from "../lib/navigation/prediction";

installGlobalErrorLog("hud");

interface PositionUpdate {
  xCm: number;
  yCm: number;
  px: number;
  py: number;
  headingDeg: number | null;
  velocityXCmS: number | null;
  velocityYCmS: number | null;
  velocityPxXS: number | null;
  velocityPxYS: number | null;
  confirmedAtMs: number;
  predictionHorizonS: number;
  staleAfterS: number;
}

interface NavigationTarget {
  id: string;
  name: string;
  xCm: number;
  yCm: number;
  distanceM: number;
  arrived: boolean;
}

const hud = document.getElementById("hud")!;
const headingEl = document.getElementById("heading")!;
const freshnessEl = document.getElementById("freshness")!;
const navigationEl = document.getElementById("navigation")!;
const targetArrowEl = document.getElementById("target-arrow")!;
const targetNameEl = document.getElementById("target-name")!;
const targetDetailEl = document.getElementById("target-detail")!;
const turnEl = document.getElementById("turn")!;

let language: HudLanguage = "vi";
let confirmedPosition: PositionUpdate | null = null;
let displayedPosition: ProjectedPosition | null = null;
let correctionFrom: ProjectedPosition | null = null;
let correctionStartedAtMs = 0;
let navigation: NavigationTarget | null = null;
let predictionFrame: number | null = null;
let staleTimer: number | null = null;
let lastPaintAtMs = 0;

function applySettings(settings: Record<string, unknown>) {
  language = settings.language === "en" ? "en" : "vi";
  const navigationSettings = settings.navigation as Record<string, unknown> | undefined;
  const opacity = Number(navigationSettings?.hud_opacity ?? 0.92);
  hud.style.opacity = String(Math.max(0.35, Math.min(1, opacity)));
}

const fmtDistance = (metres: number) =>
  metres >= 1_000 ? `${(metres / 1_000).toFixed(1)} km` : `${Math.round(metres)} m`;

function turnInstruction(relative: number, arrived: boolean): string {
  if (arrived) return language === "vi" ? "ĐÃ TỚI" : "ARRIVED";
  const degrees = Math.round(Math.abs(relative));
  if (degrees <= 7) return language === "vi" ? "ĐI THẲNG" : "STRAIGHT";
  if (relative > 0) return language === "vi" ? `RẼ PHẢI ${degrees}°` : `RIGHT ${degrees}°`;
  return language === "vi" ? `RẼ TRÁI ${degrees}°` : `LEFT ${degrees}°`;
}

function scheduleStaleRefresh(projected: ProjectedPosition) {
  if (staleTimer !== null) window.clearTimeout(staleTimer);
  staleTimer = null;
  if (!confirmedPosition || projected.stale || projected.predicting) return;
  const remainingMs = Math.max(0, (confirmedPosition.staleAfterS - projected.ageS) * 1_000);
  staleTimer = window.setTimeout(() => paint(Date.now(), true), remainingMs + 20);
}

function paint(nowMs: number, force = false) {
  predictionFrame = null;
  if (!confirmedPosition) {
    hud.classList.add("waiting");
    headingEl.textContent = language === "vi" ? "CHỜ VỊ TRÍ" : "WAITING FOR POSITION";
    navigationEl.classList.add("hidden");
    return;
  }

  // Limit DOM writes to about 30 fps; enough for a direction HUD and kinder
  // to game frame time than a second 60 fps overlay.
  if (!force && nowMs - lastPaintAtMs < 32) {
    predictionFrame = requestAnimationFrame(() => paint(Date.now()));
    return;
  }
  lastPaintAtMs = nowMs;

  const projected = projectedPosition(confirmedPosition, nowMs);
  const shown = smoothedPosition(
    projected,
    correctionFrom,
    correctionStartedAtMs,
    nowMs,
  );
  displayedPosition = shown;
  hud.classList.remove("waiting");

  if (confirmedPosition.headingDeg === null) {
    headingEl.textContent = language === "vi" ? "CHƯA RÕ HƯỚNG" : "HEADING UNKNOWN";
  } else {
    const point = compassPoint(confirmedPosition.headingDeg, language);
    const prefix = language === "vi" ? "ĐANG NHÌN" : "HEADING";
    headingEl.textContent = `${prefix}: ${point} ${Math.round(confirmedPosition.headingDeg)}°`;
  }

  freshnessEl.className = "freshness";
  if (projected.stale) {
    freshnessEl.classList.add("stale");
    freshnessEl.textContent = language === "vi" ? "MẤT TÍN HIỆU" : "STALE";
  } else if (projected.predicting) {
    freshnessEl.classList.add("predicting");
    freshnessEl.textContent = language === "vi" ? "ƯỚC TÍNH" : "ESTIMATE";
  } else {
    freshnessEl.textContent = "SERVER";
  }

  if (navigation) {
    navigationEl.classList.remove("hidden");
    navigationEl.classList.toggle("arrived", navigation.arrived);
    const bearing = bearingTo(shown.xCm, shown.yCm, navigation.xCm, navigation.yCm);
    const distance = distanceMetres(shown.xCm, shown.yCm, navigation.xCm, navigation.yCm);
    const relative = confirmedPosition.headingDeg === null
      ? 0
      : relativeBearing(confirmedPosition.headingDeg, bearing);
    targetArrowEl.style.transform = `rotate(${relative.toFixed(2)}deg)`;
    targetNameEl.textContent = navigation.name;
    targetDetailEl.textContent = `${compassPoint(bearing, language)} · ${fmtDistance(distance)}`;
    turnEl.textContent = confirmedPosition.headingDeg === null
      ? (language === "vi" ? "THEO MŨI TÊN BẢN ĐỒ" : "USE MAP ARROW")
      : turnInstruction(relative, navigation.arrived);
  } else {
    navigationEl.classList.add("hidden");
  }

  const correcting = nowMs - correctionStartedAtMs < 350;
  if (projected.predicting || correcting) {
    predictionFrame = requestAnimationFrame(() => paint(Date.now()));
  } else {
    scheduleStaleRefresh(projected);
  }
}

function acceptPosition(position: PositionUpdate) {
  if (predictionFrame !== null) cancelAnimationFrame(predictionFrame);
  if (staleTimer !== null) window.clearTimeout(staleTimer);
  correctionFrom = displayedPosition;
  correctionStartedAtMs = Date.now();
  confirmedPosition = position;
  paint(correctionStartedAtMs, true);
}

async function refreshNavigation() {
  navigation = await invoke<NavigationTarget | null>("active_navigation");
  paint(Date.now(), true);
}

async function init() {
  const settings = await invoke<Record<string, unknown>>("get_settings");
  applySettings(settings);

  await listen<PositionUpdate>("position://update", (event) => acceptPosition(event.payload));
  await listen("navigation://changed", () => void refreshNavigation());
  await listen("waypoints://changed", () => void refreshNavigation());
  await listen<Record<string, unknown>>("settings://changed", (event) => {
    applySettings(event.payload);
    paint(Date.now(), true);
  });

  navigation = await invoke<NavigationTarget | null>("active_navigation");
  const current = await invoke<PositionUpdate | null>("get_current_position");
  if (current) acceptPosition(current);
  else paint(Date.now(), true);
  await emit("hud://ready", {});
}

void init().catch((reason) => {
  void error(`[hud] init failed: ${reason}`).catch(() => {});
  void emit("hud://ready", {});
});
