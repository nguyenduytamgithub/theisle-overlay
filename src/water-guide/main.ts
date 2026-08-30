import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { error } from "@tauri-apps/plugin-log";

import { installGlobalErrorLog } from "../lib/errlog";
import {
  NavigationEstimator,
  type NavigationSnapshot,
} from "../lib/navigation/estimator";
import {
  advanceScreenAngle,
  instructionFor,
  waterGuideFrame,
  type WaterGuideLanguage,
  type WaterGuideRoute,
} from "../lib/navigation/water-guide";

installGlobalErrorLog("water-guide");

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
}

interface WaterGuideSnapshot {
  requested: boolean;
  route: WaterGuideRoute | null;
  errorKey: string | null;
}

const FRAME_MS = 1_000 / 30;
const estimator = new NavigationEstimator();
const root = document.getElementById("water-guide")!;
const destinationEl = document.getElementById("destination")!;
const instructionEl = document.getElementById("instruction")!;
const rayEl = document.getElementById("ray")!;
const uturnEl = document.getElementById("uturn")!;

let state: WaterGuideSnapshot = { requested: false, route: null, errorKey: null };
let language: WaterGuideLanguage = "vi";
let paintTimer: number | null = null;
let displayedAngleDeg = 0;
let lastPaintAt = performance.now();

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

function hideRay() {
  rayEl.classList.add("hidden");
  uturnEl.classList.add("hidden");
}

function paintError(errorKey: string | null) {
  hideRay();
  root.dataset.state = "error";
  destinationEl.textContent = "WATER GUIDE";
  instructionEl.textContent = ERROR_COPY[language][errorKey ?? ""]
    ?? (language === "vi"
      ? "KHÔNG XÁC MINH ĐƯỢC NƯỚC UỐNG"
      : "DRINKABLE WATER COULD NOT BE VERIFIED");
}

function paintView(view: NavigationSnapshot) {
  const route = state.route;
  if (!route) {
    paintError(state.errorKey);
    return;
  }

  const frame = waterGuideFrame(route, {
    xCm: view.xCm,
    yCm: view.yCm,
    guidanceCourseDeg: view.guidanceCourseDeg,
    freshness: view.freshness,
  });
  const now = performance.now();
  displayedAngleDeg = advanceScreenAngle(
    displayedAngleDeg,
    frame.screenAngleDeg,
    Math.min(0.1, Math.max(0, (now - lastPaintAt) / 1_000)),
  );
  lastPaintAt = now;

  root.dataset.state = frame.state;
  root.dataset.turn = frame.turn;
  destinationEl.textContent = (language === "vi" ? "NƯỚC: " : "WATER: ")
    + route.label + " · " + Math.round(frame.remainingM) + " m";
  instructionEl.textContent = instructionFor(frame, language);

  rayEl.classList.toggle("hidden", !frame.rayVisible);
  rayEl.style.setProperty("--ray-angle", String(displayedAngleDeg) + "deg");
  const uturn = frame.turn === "uturn" && frame.rayVisible;
  uturnEl.classList.toggle("hidden", !uturn);
  if (uturn) {
    const left = frame.relativeDeg <= 0;
    uturnEl.textContent = (left ? "↶ " : "↷ ")
      + (language === "vi" ? "QUAY ĐẦU" : "TURN AROUND");
  }
}

function paint() {
  paintTimer = null;
  root.classList.toggle("requested", state.requested);
  if (!state.requested) {
    hideRay();
    return;
  }
  if (!state.route) {
    paintError(state.errorKey);
    return;
  }
  const view = estimator.snapshot(Date.now());
  if (!view) {
    paintError("waiting_for_position");
    schedulePaint();
    return;
  }
  paintView(view);
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
  estimator.accept(position);
  paintNow();
}

async function init() {
  applySettings(await invoke<Record<string, unknown>>("get_settings"));

  await listen<PositionUpdate>("position://update", ({ payload }) => {
    acceptPosition(payload);
  });
  await listen("position://quality", () => {
    estimator.invalidatePrediction();
    paintNow();
  });
  await listen<WaterGuideSnapshot>("water-guide://changed", ({ payload }) => {
    state = payload;
    displayedAngleDeg = 0;
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
