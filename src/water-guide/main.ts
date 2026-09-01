import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { error } from "@tauri-apps/plugin-log";

import { installGlobalErrorLog } from "../lib/errlog";
import {
  freshnessForAge,
  type NavigationFreshness,
} from "../lib/navigation/estimator";
import {
  instructionFor,
  movementCourseBetween,
  nextAlignmentLocked,
  steeringPromptFor,
  waypointGuideRoute,
  waterGuideBoardNeedles,
  waterGuideFrame,
  type WaterGuideLanguage,
  type WaterGuideRoute,
} from "../lib/navigation/water-guide";

installGlobalErrorLog("water-guide");

interface PositionUpdate {
  xCm: number;
  yCm: number;
  confirmedAtMs: number;
  relocated: boolean;
}

interface WaterGuideSnapshot {
  requested: boolean;
  route: WaterGuideRoute | null;
  errorKey: string | null;
}

interface NavigationTarget {
  id: string;
  name: string;
  xCm: number;
  yCm: number;
  distanceM: number;
  arrived: boolean;
}

type GuideSource = "water" | "waypoint";

const FRAME_MS = 1_000 / 30;
const root = document.getElementById("water-guide")!;
const destinationEl = document.getElementById("destination")!;
const instructionEl = document.getElementById("instruction")!;
const boardEl = document.getElementById("board")!;
const targetNeedleEl = document.getElementById("target-needle")!;
const movementNeedleEl = document.getElementById("movement-needle")!;
const maneuverEl = document.getElementById("maneuver")!;

let waterState: WaterGuideSnapshot = { requested: false, route: null, errorKey: null };
let language: WaterGuideLanguage = "vi";
let paintTimer: number | null = null;
let alignmentLocked = false;
let latestConfirmedPosition: PositionUpdate | null = null;
let movementAnchor: PositionUpdate | null = null;
let movementCourseDeg: number | null = null;
let positionQualityValid = true;
let waterGuideStateRevision = 0;
let positionRevision = 0;
let settingsRevision = 0;
let navigationRevision = 0;
let selectedWaypointId: string | null = null;
let navigationTarget: NavigationTarget | null = null;
let waypointRoute: WaterGuideRoute | null = null;
let waypointRouteId: string | null = null;

const ERROR_COPY: Record<WaterGuideLanguage, Record<string, string>> = {
  vi: {
    waiting_for_position: "CHỜ VỊ TRÍ · CHỜ SERVER CẬP NHẬT",
    waypoint_waiting: "ĐIỂM GHIM · CHỜ VỊ TRÍ",
    missing_freshwater: "THIẾU DỮ LIỆU NƯỚC NGỌT",
    invalid_freshwater: "DỮ LIỆU NƯỚC NGỌT KHÔNG HỢP LỆ",
    unsupported_map: "BẢN ĐỒ CHƯA ĐƯỢC HỖ TRỢ",
    empty_freshwater: "KHÔNG TÌM THẤY NƯỚC UỐNG",
    missing_water_labels: "THIẾU NHÃN NGUỒN NƯỚC",
    invalid_pois: "DỮ LIỆU BẢN ĐỒ KHÔNG HỢP LỆ",
  },
  en: {
    waiting_for_position: "WAITING FOR A FRESH SERVER POSITION",
    waypoint_waiting: "WAYPOINT · WAITING FOR POSITION",
    missing_freshwater: "FRESHWATER DATA IS MISSING",
    invalid_freshwater: "FRESHWATER DATA IS INVALID",
    unsupported_map: "THIS MAP VERSION IS NOT SUPPORTED",
    empty_freshwater: "NO DRINKABLE WATER FOUND",
    missing_water_labels: "WATER LABELS ARE MISSING",
    invalid_pois: "MAP DATA IS INVALID",
  },
};

function applySettings(settings: Record<string, unknown>): boolean {
  language = settings.language === "en" ? "en" : "vi";
  document.documentElement.lang = language;
  const navigation = settings.navigation as Record<string, unknown> | undefined;
  const rawWaypointId = navigation?.target_waypoint_id;
  const nextWaypointId = typeof rawWaypointId === "string" && rawWaypointId.trim()
    ? rawWaypointId
    : null;
  const waypointChanged = nextWaypointId !== selectedWaypointId;
  if (waypointChanged) {
    selectedWaypointId = nextWaypointId;
    navigationTarget = null;
    waypointRoute = null;
    waypointRouteId = null;
    resetMovementCourse();
  }
  return waypointChanged;
}

function activeGuide(): {
  source: GuideSource;
  route: WaterGuideRoute | null;
  errorKey: string | null;
} | null {
  if (waterState.requested) {
    return { source: "water", route: waterState.route, errorKey: waterState.errorKey };
  }
  if (selectedWaypointId) {
    return { source: "waypoint", route: waypointRoute, errorKey: "waypoint_waiting" };
  }
  return null;
}

function hideBoard() {
  alignmentLocked = false;
  root.dataset.aligned = "false";
  boardEl.classList.add("hidden");
  targetNeedleEl.classList.add("hidden");
  movementNeedleEl.classList.add("hidden");
  maneuverEl.classList.add("hidden");
}

function paintError(errorKey: string | null, source: GuideSource) {
  hideBoard();
  root.dataset.state = "error";
  destinationEl.textContent = source === "waypoint" ? "ĐIỂM GHIM" : "WATER GUIDE";
  instructionEl.textContent = ERROR_COPY[language][errorKey ?? ""]
    ?? (language === "vi"
      ? "KHÔNG XÁC MINH ĐƯỢC NƯỚC UỐNG"
      : "DRINKABLE WATER COULD NOT BE VERIFIED");
}

function paintView(
  route: WaterGuideRoute,
  source: GuideSource,
  freshness: NavigationFreshness,
) {
  const position = latestConfirmedPosition;
  if (!position) return;

  const frame = waterGuideFrame(route, {
    xCm: position.xCm,
    yCm: position.yCm,
    movementCourseDeg,
    freshness,
  });
  const needles = waterGuideBoardNeedles(frame, movementCourseDeg);
  alignmentLocked = nextAlignmentLocked(alignmentLocked, frame);
  const prompt = steeringPromptFor(frame, alignmentLocked, language);

  root.dataset.state = frame.state;
  root.dataset.turn = frame.turn;
  root.dataset.aligned = String(alignmentLocked);
  const prefix = source === "waypoint"
    ? (language === "vi" ? "ĐIỂM: " : "WAYPOINT: ")
    : (language === "vi" ? "NƯỚC: " : "WATER: ");
  destinationEl.textContent = prefix
    + route.label + " · " + Math.round(frame.remainingM) + " m";
  instructionEl.textContent = alignmentLocked
    ? prompt
    : instructionFor(frame, language);

  boardEl.classList.toggle("hidden", !needles.targetVisible);
  targetNeedleEl.classList.toggle("hidden", !needles.targetVisible);
  movementNeedleEl.classList.toggle("hidden", !needles.movementVisible);
  if (needles.targetBearingDeg !== null) {
    boardEl.style.setProperty(
      "--target-bearing",
      `${needles.targetBearingDeg}deg`,
    );
  }
  if (needles.movementBearingDeg !== null) {
    boardEl.style.setProperty(
      "--movement-bearing",
      `${needles.movementBearingDeg}deg`,
    );
  }
  maneuverEl.classList.toggle("hidden", !needles.targetVisible);
  if (needles.targetVisible) {
    maneuverEl.textContent = prompt;
  }
}

function paint() {
  paintTimer = null;
  const guide = activeGuide();
  root.classList.toggle("requested", guide !== null);
  if (!guide) {
    hideBoard();
    return;
  }
  if (!guide.route) {
    paintError(guide.errorKey, guide.source);
    return;
  }
  if (!latestConfirmedPosition) {
    paintError(
      guide.source === "waypoint" ? "waypoint_waiting" : "waiting_for_position",
      guide.source,
    );
    schedulePaint();
    return;
  }
  if (guide.source === "waypoint" && navigationTarget?.arrived) {
    hideBoard();
    root.dataset.state = "arrived";
    destinationEl.textContent = (language === "vi" ? "ĐIỂM: " : "WAYPOINT: ")
      + navigationTarget.name + " · " + Math.round(navigationTarget.distanceM) + " m";
    instructionEl.textContent = language === "vi" ? "ĐÃ TỚI ĐIỂM GHIM" : "WAYPOINT REACHED";
    schedulePaint();
    return;
  }
  const ageS = Math.max(
    0,
    (Date.now() - latestConfirmedPosition.confirmedAtMs) / 1_000,
  );
  paintView(
    guide.route,
    guide.source,
    positionQualityValid ? freshnessForAge(ageS) : "waiting",
  );
  schedulePaint();
}

function schedulePaint() {
  if (paintTimer === null) {
    paintTimer = window.setTimeout(paint, FRAME_MS);
  }
}

function paintNow() {
  if (paintTimer !== null) {
    window.clearTimeout(paintTimer);
    paintTimer = null;
  }
  paint();
}

function acceptPosition(position: PositionUpdate) {
  if (
    latestConfirmedPosition
    && position.confirmedAtMs < latestConfirmedPosition.confirmedAtMs
  ) {
    return;
  }
  positionQualityValid = true;
  if (position.relocated || movementAnchor === null) {
    movementAnchor = position;
    movementCourseDeg = null;
  } else {
    const course = movementCourseBetween(movementAnchor, position);
    if (course !== null) {
      movementCourseDeg = course;
      movementAnchor = position;
    }
  }
  latestConfirmedPosition = position;
  ensureWaypointRoute();
  paintNow();
  if (selectedWaypointId && !navigationTarget) {
    void refreshNavigation();
  }
}

function resetMovementCourse() {
  movementCourseDeg = null;
  movementAnchor = latestConfirmedPosition;
  alignmentLocked = false;
}

function ensureWaypointRoute() {
  if (!navigationTarget || !latestConfirmedPosition || !selectedWaypointId) {
    return;
  }
  if (navigationTarget.id !== selectedWaypointId) {
    return;
  }
  if (!waypointRoute || waypointRouteId !== navigationTarget.id) {
    waypointRoute = waypointGuideRoute(navigationTarget, latestConfirmedPosition);
    waypointRouteId = navigationTarget.id;
    return;
  }
  waypointRoute = {
    ...waypointRoute,
    targetXCm: navigationTarget.xCm,
    targetYCm: navigationTarget.yCm,
    label: navigationTarget.name,
    initialDistanceM: navigationTarget.distanceM,
  };
}

function applyNavigationTarget(target: NavigationTarget | null) {
  const accepted = target && target.id === selectedWaypointId ? target : null;
  const targetChanged = navigationTarget?.id !== accepted?.id;
  navigationTarget = accepted;
  if (targetChanged) {
    waypointRoute = null;
    waypointRouteId = null;
    resetMovementCourse();
  }
  ensureWaypointRoute();
  paintNow();
}

async function refreshNavigation() {
  const revisionBeforeRequest = navigationRevision;
  const waypointIdBeforeRequest = selectedWaypointId;
  const target = await invoke<NavigationTarget | null>("active_navigation");
  if (
    navigationRevision !== revisionBeforeRequest
    || selectedWaypointId !== waypointIdBeforeRequest
  ) {
    return;
  }
  applyNavigationTarget(target);
}

async function init() {
  await listen<PositionUpdate>("position://update", ({ payload }) => {
    positionRevision += 1;
    acceptPosition(payload);
  });
  await listen("position://quality", () => {
    positionRevision += 1;
    positionQualityValid = false;
    alignmentLocked = false;
    paintNow();
  });
  await listen<WaterGuideSnapshot>("water-guide://changed", ({ payload }) => {
    waterGuideStateRevision += 1;
    waterState = payload;
    resetMovementCourse();
    paintNow();
  });
  await listen("navigation://changed", () => {
    navigationRevision += 1;
    void refreshNavigation();
  });
  await listen("waypoints://changed", () => {
    navigationRevision += 1;
    void refreshNavigation();
  });
  await listen<Record<string, unknown>>("settings://changed", ({ payload }) => {
    settingsRevision += 1;
    if (applySettings(payload)) {
      navigationRevision += 1;
      void refreshNavigation();
    }
    paintNow();
  });

  const settingsRevisionBeforeSnapshot = settingsRevision;
  const initialSettings = await invoke<Record<string, unknown>>("get_settings");
  if (
    settingsRevisionBeforeSnapshot === 0
    && settingsRevision === settingsRevisionBeforeSnapshot
  ) {
    applySettings(initialSettings);
  }

  const stateRevisionBeforeSnapshot = waterGuideStateRevision;
  const initialState = await invoke<WaterGuideSnapshot>("get_water_guide_state");
  if (
    stateRevisionBeforeSnapshot === 0
    && waterGuideStateRevision === stateRevisionBeforeSnapshot
  ) {
    waterState = initialState;
  }

  const navigationRevisionBeforeSnapshot = navigationRevision;
  const initialNavigation = await invoke<NavigationTarget | null>("active_navigation");
  if (
    navigationRevisionBeforeSnapshot === 0
    && navigationRevision === navigationRevisionBeforeSnapshot
  ) {
    applyNavigationTarget(initialNavigation);
  }

  const positionRevisionBeforeSnapshot = positionRevision;
  const current = await invoke<PositionUpdate | null>("get_current_position");
  if (
    positionRevisionBeforeSnapshot === 0
    && positionRevision === positionRevisionBeforeSnapshot
    && current
  ) {
    acceptPosition(current);
  } else {
    paintNow();
  }
  await emit("water-guide://ready");
}

void init().catch((reason) => {
  void error("[water-guide] init failed: " + String(reason)).catch(() => {});
  void emit("water-guide://ready");
});
