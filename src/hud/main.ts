import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { error } from "@tauri-apps/plugin-log";

import { installGlobalErrorLog } from "../lib/errlog";
import { compassPoint, type HudLanguage } from "../lib/navigation/guidance";
import {
  localizeFreshness,
  localizeManeuver,
  NavigationEstimator,
  type NavigationSnapshot,
} from "../lib/navigation/estimator";

installGlobalErrorLog("hud");

interface PositionUpdate {
  xCm: number;
  yCm: number;
  px: number;
  py: number;
  velocityXCmS: number | null;
  velocityYCmS: number | null;
  velocityPxXS: number | null;
  velocityPxYS: number | null;
  serverFacingDeg: number | null;
  motionCourseDeg: number | null;
  confirmedAtMs: number;
  relocated: boolean;
  refreshedOnly: boolean;
}

interface NavigationTarget {
  id: string;
  name: string;
  xCm: number;
  yCm: number;
  distanceM: number;
  arrived: boolean;
}

const FRAME_MS = 1_000 / 30;
const estimator = new NavigationEstimator();
const hud = document.getElementById("hud")!;
const courseEl = document.getElementById("course")!;
const freshnessEl = document.getElementById("freshness")!;
const navigationEl = document.getElementById("navigation")!;
const targetArrowEl = document.getElementById("target-arrow")!;
const targetNameEl = document.getElementById("target-name")!;
const targetDetailEl = document.getElementById("target-detail")!;
const instructionEl = document.getElementById("instruction")!;
const progressWarningEl = document.getElementById("progress-warning")!;

let language: HudLanguage = "vi";
let navigation: NavigationTarget | null = null;
let paintTimer: number | null = null;
let hasPosition = false;

function applySettings(settings: Record<string, unknown>) {
  language = settings.language === "en" ? "en" : "vi";
  const navigationSettings = settings.navigation as Record<string, unknown> | undefined;
  const opacity = Number(navigationSettings?.hud_opacity ?? 0.92);
  hud.style.opacity = String(Math.max(0.35, Math.min(1, opacity)));
}

const fmtDistance = (metres: number) =>
  metres >= 1_000 ? `${(metres / 1_000).toFixed(1)} km` : `${Math.round(metres)} m`;

function schedulePaint() {
  if (paintTimer !== null) window.clearTimeout(paintTimer);
  paintTimer = window.setTimeout(() => paint(Date.now()), FRAME_MS);
}

function paintNow() {
  if (paintTimer !== null) window.clearTimeout(paintTimer);
  paintTimer = null;
  paint(Date.now());
}

function paintCourse(view: NavigationSnapshot) {
  if (view.guidanceCourseDeg === null) {
    courseEl.textContent = language === "vi" ? "HƯỚNG ĐI: CHƯA RÕ" : "COURSE: UNKNOWN";
    return;
  }
  const point = compassPoint(view.guidanceCourseDeg, language);
  const prefix = language === "vi" ? "HƯỚNG ĐI" : "COURSE";
  courseEl.textContent = `${prefix}: ${point} ${Math.round(view.guidanceCourseDeg)}°`;
}

function paintFreshness(view: NavigationSnapshot) {
  freshnessEl.className = `freshness ${view.freshness}`;
  freshnessEl.textContent = localizeFreshness(view.freshness, language);
}

function paintNavigation(view: NavigationSnapshot) {
  if (!navigation || view.targetBearingDeg === null || view.targetDistanceM === null) {
    navigationEl.classList.add("hidden");
    return;
  }

  navigationEl.classList.remove("hidden");
  navigationEl.classList.toggle("arrived", view.arrived);
  // This arrow is north-up and absolute. Raw server yaw can never rotate it.
  if (!view.arrived) {
    targetArrowEl.style.transform = `rotate(${view.targetBearingDeg.toFixed(2)}deg)`;
  }
  const cardinal = compassPoint(view.targetBearingDeg, language);
  targetNameEl.textContent = navigation.name;
  targetDetailEl.textContent = `${cardinal} · ${fmtDistance(view.targetDistanceM)}`;
  instructionEl.textContent = localizeManeuver(view.maneuver, language, cardinal);

  progressWarningEl.classList.toggle("hidden", !view.noProgress || view.arrived);
  progressWarningEl.textContent = language === "vi"
    ? "ĐANG ĐI XA ĐÍCH — KIỂM TRA BẢN ĐỒ"
    : "NO PROGRESS — CHECK THE MAP";
}

function paint(nowMs: number) {
  paintTimer = null;
  const view = estimator.snapshot(nowMs);
  if (!view) {
    hud.classList.add("waiting");
    courseEl.textContent = language === "vi" ? "CHỜ VỊ TRÍ" : "WAITING FOR POSITION";
    freshnessEl.textContent = language === "vi" ? "CHỜ SERVER" : "WAITING FOR SERVER";
    navigationEl.classList.add("hidden");
    return;
  }

  hud.classList.remove("waiting");
  paintCourse(view);
  paintFreshness(view);
  paintNavigation(view);
  schedulePaint();
}

function acceptPosition(position: PositionUpdate) {
  hasPosition = true;
  estimator.accept(position);
  paintNow();
}

async function refreshNavigation() {
  navigation = await invoke<NavigationTarget | null>("active_navigation");
  estimator.setTarget(navigation
    ? {
        id: navigation.id,
        name: navigation.name,
        xCm: navigation.xCm,
        yCm: navigation.yCm,
      }
    : null);
  paintNow();
}

async function init() {
  const settings = await invoke<Record<string, unknown>>("get_settings");
  applySettings(settings);

  await listen<PositionUpdate>("position://update", (event) => {
    acceptPosition(event.payload);
    // A persisted target can become resolvable on the first confirmed sample.
    if (!navigation) void refreshNavigation();
  });
  await listen("navigation://changed", () => void refreshNavigation());
  await listen("waypoints://changed", () => void refreshNavigation());
  await listen<Record<string, unknown>>("settings://changed", (event) => {
    applySettings(event.payload);
    paintNow();
  });

  navigation = await invoke<NavigationTarget | null>("active_navigation");
  estimator.setTarget(navigation
    ? {
        id: navigation.id,
        name: navigation.name,
        xCm: navigation.xCm,
        yCm: navigation.yCm,
      }
    : null);
  const current = await invoke<PositionUpdate | null>("get_current_position");
  if (current) acceptPosition(current);
  else paintNow();
  await emit("hud://ready", { hasPosition });
}

void init().catch((reason) => {
  void error(`[hud] init failed: ${reason}`).catch(() => {});
  void emit("hud://ready", { hasPosition: false });
});
