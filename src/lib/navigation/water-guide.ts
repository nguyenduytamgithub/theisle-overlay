import { bearingTo, relativeBearing } from "./guidance.ts";
import type { NavigationFreshness } from "./estimator.ts";

export const WATER_GUIDE = {
  onRouteM: 15,
  lostM: 150,
  arrivalM: 25,
  lookAheadM: 80,
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
    screenAngleDeg: Math.max(-75, Math.min(75, relativeDeg)),
    turn,
  };
}
