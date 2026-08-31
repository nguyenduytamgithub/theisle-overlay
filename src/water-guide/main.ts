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

const FRAME_MS = 1_000 / 30;
const root = document.getElementById("water-guide")!;
const destinationEl = document.getElementById("destination")!;
const instructionEl = document.getElementById("instruction")!;
const boardEl = document.getElementById("board")!;
const targetNeedleEl = document.getElementById("target-needle")!;
const movementNeedleEl = document.getElementById("movement-needle")!;
const maneuverEl = document.getElementById("maneuver")!;

let state: WaterGuideSnapshot = { requested: false, route: null, errorKey: null };
let language: WaterGuideLanguage = "vi";
let paintTimer: number | null = null;
let alignmentLocked = false;
let latestConfirmedPosition: PositionUpdate | null = null;
let movementAnchor: PositionUpdate | null = null;
let movementCourseDeg: number | null = null;
let positionQualityValid = true;

const ERROR_COPY: Record<WaterGuideLanguage, Record<string, string>> = {
  vi: {
    waiting_for_position: "CHỜ VỊ TRÍ · CHỜ SERVER CẬP NHẬT",
    missing_freshwater: "THIẾU DỮ LIỆU NƯỚC NGỌT",
    invalid_freshwater: "DỮ LIỆU NƯỚC NGỌT KHÔNG HỢP LỆ",
    unsupported_map: "BẢN ĐỒ CHƯA ĐƯỢC HỖ TRỢ",
    empty_freshwater: "KHÔNG TÌM THẤY NƯỚC UỐNG",
    missing_water_labels: "THIẾU NHÃN NGUỒN NƯỚC",
    invalid_pois: "DỮ LIỆU BẢN ĐỒ KHÔNG HỢP LỆ",
  },
  en: {
    waiting_for_position: "WAITING FOR A FRESH SERVER POSITION",
    missing_freshwater: "FRESHWATER DATA IS MISSING",
    invalid_freshwater: "FRESHWATER DATA IS INVALID",
    unsupported_map: "THIS MAP VERSION IS NOT SUPPORTED",
    empty_freshwater: "NO DRINKABLE WATER FOUND",
    missing_water_labels: "WATER LABELS ARE MISSING",
    invalid_pois: "MAP DATA IS INVALID",
  },
};

function applySettings(settings: Record<string, unknown>) {
  language = settings.language === "en" ? "en" : "vi";
  document.documentElement.lang = language;
}

function hideBoard() {
  alignmentLocked = false;
  root.dataset.aligned = "false";
  boardEl.classList.add("hidden");
  targetNeedleEl.classList.add("hidden");
  movementNeedleEl.classList.add("hidden");
  maneuverEl.classList.add("hidden");
}

function paintError(errorKey: string | null) {
  hideBoard();
  root.dataset.state = "error";
  destinationEl.textContent = "WATER GUIDE";
  instructionEl.textContent = ERROR_COPY[language][errorKey ?? ""]
    ?? (language === "vi"
      ? "KHÔNG XÁC MINH ĐƯỢC NƯỚC UỐNG"
      : "DRINKABLE WATER COULD NOT BE VERIFIED");
}

function paintView(freshness: NavigationFreshness) {
  const route = state.route;
  if (!route || !latestConfirmedPosition) {
    paintError(state.errorKey);
    return;
  }

  const frame = waterGuideFrame(route, {
    xCm: latestConfirmedPosition.xCm,
    yCm: latestConfirmedPosition.yCm,
    movementCourseDeg,
    freshness,
  });
  const needles = waterGuideBoardNeedles(frame, movementCourseDeg);
  alignmentLocked = nextAlignmentLocked(alignmentLocked, frame);
  const prompt = steeringPromptFor(frame, alignmentLocked, language);

  root.dataset.state = frame.state;
  root.dataset.turn = frame.turn;
  root.dataset.aligned = String(alignmentLocked);
  destinationEl.textContent = (language === "vi" ? "NƯỚC: " : "WATER: ")
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
  root.classList.toggle("requested", state.requested);
  if (!state.requested) {
    hideBoard();
    return;
  }
  if (!state.route) {
    paintError(state.errorKey);
    return;
  }
  if (!latestConfirmedPosition) {
    paintError("waiting_for_position");
    schedulePaint();
    return;
  }
  const ageS = Math.max(
    0,
    (Date.now() - latestConfirmedPosition.confirmedAtMs) / 1_000,
  );
  paintView(positionQualityValid ? freshnessForAge(ageS) : "waiting");
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
  paintNow();
}

function resetMovementCourse() {
  movementCourseDeg = null;
  movementAnchor = latestConfirmedPosition;
  alignmentLocked = false;
}

async function init() {
  applySettings(await invoke<Record<string, unknown>>("get_settings"));

  await listen<PositionUpdate>("position://update", ({ payload }) => {
    acceptPosition(payload);
  });
  await listen("position://quality", () => {
    positionQualityValid = false;
    alignmentLocked = false;
    paintNow();
  });
  await listen<WaterGuideSnapshot>("water-guide://changed", ({ payload }) => {
    state = payload;
    resetMovementCourse();
    paintNow();
  });
  await listen<Record<string, unknown>>("settings://changed", ({ payload }) => {
    applySettings(payload);
    paintNow();
  });

  state = await invoke<WaterGuideSnapshot>("get_water_guide_state");
  const current = await invoke<PositionUpdate | null>("get_current_position");
  if (current) acceptPosition(current);
  else paintNow();
  await emit("water-guide://ready");
}

void init().catch((reason) => {
  void error("[water-guide] init failed: " + String(reason)).catch(() => {});
  void emit("water-guide://ready");
});
