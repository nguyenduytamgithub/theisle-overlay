import { bearingTo, relativeBearing } from "./guidance.ts";
import type { NavigationFreshness } from "./estimator.ts";

export const WATER_GUIDE = {
  onRouteM: 15,
  lostM: 150,
  arrivalM: 25,
  alignEnterDeg: 8,
  alignExitDeg: 18,
  uturnDeg: 110,
  movementMinCm: 500,
} as const;

export interface WaterGuideRoute {
  startXCm: number;
  startYCm: number;
  targetXCm: number;
  targetYCm: number;
  targetMaskPx: [number, number];
  label: string;
  initialDistanceM: number;
}

export interface WaterGuideView {
  xCm: number;
  yCm: number;
  movementCourseDeg: number | null;
  freshness: NavigationFreshness;
}

export type WaterGuideState =
  | "on-route"
  | "off-route"
  | "lost"
  | "waiting"
  | "movement-unknown"
  | "arrived"
  | "invalid";

export interface WaterGuideFrame {
  state: WaterGuideState;
  rayVisible: boolean;
  steeringTargetCm: [number, number] | null;
  remainingM: number;
  crossTrackM: number;
  desiredBearingDeg: number | null;
  relativeDeg: number;
  screenAngleDeg: number;
  turn: "left" | "right" | "straight" | "uturn" | "none";
}

export interface SegmentProjection {
  pointCm: [number, number];
  t: number;
  crossTrackM: number;
}

export type WaterGuideLanguage = "vi" | "en";

const INSTRUCTIONS: Record<WaterGuideLanguage, Record<WaterGuideState, string>> = {
  vi: {
    "on-route": "TIA CỐ ĐỊNH · LÀM THEO MŨI TÊN",
    "off-route": "LỆCH ĐƯỜNG · LÀM THEO MŨI TÊN",
    lost: "LẠC XA · LÀM THEO MŨI TÊN",
    waiting: "CHỜ TỌA ĐỘ MỚI · TIA GIỮ NGUYÊN",
    "movement-unknown": "ĐI VÀI BƯỚC · CHỈ DÙNG TỌA ĐỘ XY",
    arrived: "ĐÃ TỚI NGUỒN NƯỚC",
    invalid: "KHÔNG XÁC MINH ĐƯỢC NƯỚC UỐNG",
  },
  en: {
    "on-route": "FIXED RAY · FOLLOW THE TURN ARROW",
    "off-route": "OFF ROUTE · FOLLOW THE TURN ARROW",
    lost: "FAR OFF ROUTE · FOLLOW THE TURN ARROW",
    waiting: "WAITING FOR NEW COORDINATES · RAY FROZEN",
    "movement-unknown": "WALK A FEW STEPS · XY POSITION ONLY",
    arrived: "FRESH WATER REACHED",
    invalid: "DRINKABLE WATER COULD NOT BE VERIFIED",
  },
};

export function instructionFor(
  frame: WaterGuideFrame,
  language: WaterGuideLanguage,
): string {
  if (frame.turn === "uturn" && frame.rayVisible) {
    return language === "vi" ? "QUAY ĐẦU" : "TURN AROUND";
  }
  return INSTRUCTIONS[language][frame.state];
}

export function nextAlignmentLocked(
  previous: boolean,
  frame: WaterGuideFrame,
): boolean {
  if (frame.turn === "none"
      || !frame.rayVisible
      || !Number.isFinite(frame.relativeDeg)) {
    return false;
  }
  const limit = previous ? WATER_GUIDE.alignExitDeg : WATER_GUIDE.alignEnterDeg;
  return Math.abs(frame.relativeDeg) <= limit;
}

export function steeringPromptFor(
  frame: WaterGuideFrame,
  aligned: boolean,
  language: WaterGuideLanguage,
): string {
  if (!frame.rayVisible) {
    return instructionFor(frame, language);
  }
  if (frame.turn === "none") {
    return instructionFor(frame, language);
  }
  if (aligned || Math.abs(frame.relativeDeg) <= WATER_GUIDE.alignEnterDeg) {
    return language === "vi"
      ? "✓ ĐÚNG HƯỚNG · GIỮ W"
      : "✓ ON COURSE · HOLD W";
  }

  const degrees = Math.round(Math.abs(frame.relativeDeg));
  const left = frame.relativeDeg < 0;
  if (frame.turn === "uturn") {
    if (language === "vi") {
      return `QUAY ĐẦU ${degrees}°`;
    }
    return `TURN AROUND ${degrees}°`;
  }
  if (language === "vi") {
    return left
      ? `QUỸ ĐẠO XY: RẼ TRÁI ${degrees}°`
      : `QUỸ ĐẠO XY: RẼ PHẢI ${degrees}°`;
  }
  return left
    ? `TURN CHARACTER LEFT ${degrees}°`
    : `TURN CHARACTER RIGHT ${degrees}°`;
}

const stable = (value: number): number =>
  Math.round(value * 1_000_000) / 1_000_000;

const finite = (...values: number[]): boolean => values.every(Number.isFinite);

const distanceM = (from: [number, number], to: [number, number]): number =>
  Math.hypot(to[0] - from[0], to[1] - from[1]) / 100;

export interface XYPoint {
  xCm: number;
  yCm: number;
}

/** A course derived only from confirmed coordinate displacement. */
export function movementCourseBetween(
  anchor: XYPoint,
  current: XYPoint,
): number | null {
  if (!finite(anchor.xCm, anchor.yCm, current.xCm, current.yCm)) {
    return null;
  }
  const movedCm = Math.hypot(
    current.xCm - anchor.xCm,
    current.yCm - anchor.yCm,
  );
  if (movedCm < WATER_GUIDE.movementMinCm) {
    return null;
  }
  return stable(bearingTo(
    anchor.xCm,
    anchor.yCm,
    current.xCm,
    current.yCm,
  ));
}

export function projectToSegment(
  pointCm: [number, number],
  startCm: [number, number],
  targetCm: [number, number],
): SegmentProjection {
  const dx = targetCm[0] - startCm[0];
  const dy = targetCm[1] - startCm[1];
  const lengthSquared = dx * dx + dy * dy;
  const rawT = lengthSquared > 0
    ? ((pointCm[0] - startCm[0]) * dx + (pointCm[1] - startCm[1]) * dy)
      / lengthSquared
    : 0;
  const t = Math.max(0, Math.min(1, rawT));
  const projected: [number, number] = [
    stable(startCm[0] + dx * t),
    stable(startCm[1] + dy * t),
  ];
  return {
    pointCm: projected,
    t: stable(t),
    crossTrackM: stable(distanceM(pointCm, projected)),
  };
}

const emptyFrame = (state: WaterGuideState, remainingM = Number.POSITIVE_INFINITY): WaterGuideFrame => ({
  state,
  rayVisible: false,
  steeringTargetCm: null,
  remainingM,
  crossTrackM: 0,
  desiredBearingDeg: null,
  relativeDeg: 0,
  screenAngleDeg: 0,
  turn: "none",
});

export function waterGuideFrame(
  route: WaterGuideRoute,
  view: WaterGuideView,
): WaterGuideFrame {
  const target: [number, number] = [route.targetXCm, route.targetYCm];
  const point: [number, number] = [view.xCm, view.yCm];
  if (!finite(...target, ...point)) {
    return emptyFrame("invalid");
  }

  const remainingM = stable(distanceM(point, target));
  if (remainingM <= WATER_GUIDE.arrivalM) {
    return emptyFrame("arrived", remainingM);
  }
  // Owner decision: never pull the ray toward the old route. Every accepted
  // XY sample creates a fresh straight segment to the locked water target, so
  // an old cross-track distance can never label the new direct line as lost.
  const state: WaterGuideState = "on-route";
  const steeringTargetCm: [number, number] = target;
  const desiredBearingDeg = bearingTo(
    point[0],
    point[1],
    steeringTargetCm[0],
    steeringTargetCm[1],
  );
  const fixedUnknownFrame = (state: "waiting" | "movement-unknown"): WaterGuideFrame => ({
    state,
    rayVisible: true,
    steeringTargetCm,
    remainingM,
    crossTrackM: 0,
    desiredBearingDeg,
    relativeDeg: 0,
    screenAngleDeg: 0,
    turn: "none",
  });
  if (view.freshness === "waiting") {
    return fixedUnknownFrame("waiting");
  }
  if (view.movementCourseDeg === null
      || !Number.isFinite(view.movementCourseDeg)) {
    return fixedUnknownFrame("movement-unknown");
  }
  const relativeDeg = stable(relativeBearing(view.movementCourseDeg, desiredBearingDeg));
  const magnitude = Math.abs(relativeDeg);
  const turn = magnitude > WATER_GUIDE.uturnDeg
    ? "uturn"
    : magnitude <= 12 ? "straight" : relativeDeg < 0 ? "left" : "right";

  return {
    state,
    rayVisible: true,
    steeringTargetCm,
    remainingM,
    crossTrackM: 0,
    desiredBearingDeg,
    relativeDeg,
    screenAngleDeg: 0,
    turn,
  };
}
