import { bearingTo, relativeBearing } from "./guidance.ts";
import type { NavigationFreshness } from "./estimator.ts";

export const WATER_GUIDE = {
  onRouteM: 15,
  lostM: 150,
  arrivalM: 25,
  lookAheadM: 80,
  alignEnterDeg: 8,
  alignExitDeg: 18,
  uturnDeg: 110,
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
  guidanceCourseDeg: number | null;
  freshness: NavigationFreshness;
}

export type WaterGuideState =
  | "on-route"
  | "off-route"
  | "lost"
  | "waiting"
  | "heading-unknown"
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
    "on-route": "XOAY NHÂN VẬT ĐỂ TIA THẲNG LÊN · GIỮ W",
    "off-route": "LỆCH ĐƯỜNG · XOAY NHÂN VẬT VỀ TIA",
    lost: "LẠC XA · XOAY NHÂN VẬT VỀ TIA",
    waiting: "CHỜ SERVER",
    "heading-unknown": "XOAY / ĐI VÀI BƯỚC ĐỂ XÁC ĐỊNH HƯỚNG",
    arrived: "ĐÃ TỚI NGUỒN NƯỚC",
    invalid: "KHÔNG XÁC MINH ĐƯỢC NƯỚC UỐNG",
  },
  en: {
    "on-route": "TURN THE CHARACTER UNTIL THE RAY POINTS UP · HOLD W",
    "off-route": "OFF ROUTE · TURN THE CHARACTER BACK TO THE RAY",
    lost: "FAR OFF ROUTE · TURN THE CHARACTER BACK TO THE RAY",
    waiting: "WAITING FOR SERVER",
    "heading-unknown": "TURN / WALK A FEW STEPS TO FIND DIRECTION",
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
  if (!frame.rayVisible || !Number.isFinite(frame.relativeDeg)) {
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
  if (aligned || Math.abs(frame.relativeDeg) <= WATER_GUIDE.alignEnterDeg) {
    return language === "vi"
      ? "✓ ĐÚNG HƯỚNG · GIỮ W"
      : "✓ ON COURSE · HOLD W";
  }

  const degrees = Math.round(Math.abs(frame.relativeDeg));
  const left = frame.relativeDeg < 0;
  if (frame.turn === "uturn") {
    if (language === "vi") {
      return left ? `↶ QUAY ĐẦU ${degrees}°` : `QUAY ĐẦU ${degrees}° ↷`;
    }
    return left ? `↶ TURN AROUND ${degrees}°` : `TURN AROUND ${degrees}° ↷`;
  }
  if (language === "vi") {
    return left
      ? `← XOAY NHÂN VẬT TRÁI ${degrees}°`
      : `XOAY NHÂN VẬT PHẢI ${degrees}° →`;
  }
  return left
    ? `← TURN CHARACTER LEFT ${degrees}°`
    : `TURN CHARACTER RIGHT ${degrees}° →`;
}

export function advanceScreenAngle(
  currentDeg: number,
  targetDeg: number,
  elapsedS: number,
  maxRateDegS = 180,
): number {
  const delta = targetDeg - currentDeg;
  const step = Math.sign(delta)
    * Math.min(Math.abs(delta), Math.max(0, elapsedS) * maxRateDegS);
  return stable(currentDeg + step);
}

const stable = (value: number): number =>
  Math.round(value * 1_000_000) / 1_000_000;

const finite = (...values: number[]): boolean => values.every(Number.isFinite);

const distanceM = (from: [number, number], to: [number, number]): number =>
  Math.hypot(to[0] - from[0], to[1] - from[1]) / 100;

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
  const start: [number, number] = [route.startXCm, route.startYCm];
  const target: [number, number] = [route.targetXCm, route.targetYCm];
  const point: [number, number] = [view.xCm, view.yCm];
  if (!finite(...start, ...target, ...point)
      || distanceM(start, target) === 0) {
    return emptyFrame("invalid");
  }

  const remainingM = stable(distanceM(point, target));
  if (view.freshness === "waiting") {
    return emptyFrame("waiting", remainingM);
  }
  if (remainingM <= WATER_GUIDE.arrivalM) {
    return emptyFrame("arrived", remainingM);
  }
  if (view.guidanceCourseDeg === null || !Number.isFinite(view.guidanceCourseDeg)) {
    return emptyFrame("heading-unknown", remainingM);
  }

  const projection = projectToSegment(point, start, target);
  const routeLengthCm = Math.hypot(target[0] - start[0], target[1] - start[1]);
  const state: WaterGuideState = projection.crossTrackM > WATER_GUIDE.lostM
    ? "lost"
    : projection.crossTrackM > WATER_GUIDE.onRouteM ? "off-route" : "on-route";
  const steeringTargetCm: [number, number] = state === "on-route"
    ? target
    : (() => {
        const lookAheadT = Math.min(
          1,
          projection.t + (WATER_GUIDE.lookAheadM * 100) / routeLengthCm,
        );
        return [
          stable(start[0] + (target[0] - start[0]) * lookAheadT),
          stable(start[1] + (target[1] - start[1]) * lookAheadT),
        ];
      })();
  const desiredBearingDeg = bearingTo(
    point[0],
    point[1],
    steeringTargetCm[0],
    steeringTargetCm[1],
  );
  const relativeDeg = stable(relativeBearing(view.guidanceCourseDeg, desiredBearingDeg));
  const magnitude = Math.abs(relativeDeg);
  const turn = magnitude > WATER_GUIDE.uturnDeg
    ? "uturn"
    : magnitude <= 12 ? "straight" : relativeDeg < 0 ? "left" : "right";

  return {
    state,
    rayVisible: true,
    steeringTargetCm,
    remainingM,
    crossTrackM: projection.crossTrackM,
    desiredBearingDeg,
    relativeDeg,
    screenAngleDeg: 0,
    turn,
  };
}
