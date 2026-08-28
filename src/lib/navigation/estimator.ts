export type NavigationFreshness = "tracking" | "estimating" | "waiting";
export type NavigationLanguage = "vi" | "en";

export interface ConfirmedNavigationSample {
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

export interface EstimatorTarget {
  id: string;
  name: string;
  xCm: number;
  yCm: number;
}

export type NavigationManeuver =
  | "straight"
  | "slight-left"
  | "slight-right"
  | "left"
  | "right"
  | "turn-back"
  | "hold-cardinal"
  | "arrived";

const MANEUVER_COPY: Record<
  NavigationLanguage,
  Record<Exclude<NavigationManeuver, "hold-cardinal">, string>
> = {
  vi: {
    straight: "ĐI THẲNG",
    "slight-left": "CHẾCH TRÁI",
    "slight-right": "CHẾCH PHẢI",
    left: "RẼ TRÁI",
    right: "RẼ PHẢI",
    "turn-back": "QUAY LẠI",
    arrived: "ĐÃ TỚI KHU VỰC ĐÍCH",
  },
  en: {
    straight: "GO STRAIGHT",
    "slight-left": "BEAR LEFT",
    "slight-right": "BEAR RIGHT",
    left: "TURN LEFT",
    right: "TURN RIGHT",
    "turn-back": "TURN AROUND",
    arrived: "DESTINATION AREA REACHED",
  },
};

const FRESHNESS_COPY: Record<
  NavigationLanguage,
  Record<NavigationFreshness, string>
> = {
  vi: {
    tracking: "ĐANG BÁM",
    estimating: "ĐANG ƯỚC LƯỢNG",
    waiting: "CHỜ SERVER",
  },
  en: {
    tracking: "TRACKING",
    estimating: "ESTIMATING",
    waiting: "WAITING FOR SERVER",
  },
};

export function localizeManeuver(
  maneuver: NavigationManeuver,
  language: NavigationLanguage,
  cardinal?: string,
): string {
  if (maneuver === "hold-cardinal") {
    const prefix = language === "vi" ? "GIỮ HƯỚNG" : "HOLD";
    return cardinal ? `${prefix} ${cardinal}` : prefix;
  }
  return MANEUVER_COPY[language][maneuver];
}

export function localizeFreshness(
  freshness: NavigationFreshness,
  language: NavigationLanguage,
): string {
  return FRESHNESS_COPY[language][freshness];
}

export interface NavigationSnapshot {
  xCm: number;
  yCm: number;
  px: number;
  py: number;
  targetBearingDeg: number | null;
  guidanceCourseDeg: number | null;
  targetDistanceM: number | null;
  maneuver: NavigationManeuver;
  freshness: NavigationFreshness;
  predicting: boolean;
  arrived: boolean;
  noProgress: boolean;
}

type Coordinates = Pick<NavigationSnapshot, "xCm" | "yCm" | "px" | "py">;
type CourseSource = "motion" | "server";

const DEFAULT_ARRIVAL_RADIUS_M = 25;
const ARRIVAL_EXIT_MARGIN_M = 5;
const COURSE_SOURCE_STABLE_MS = 1_000;
const MANEUVER_STABLE_MS = 600;
const MANEUVER_TIMELINE_STEP_MS = 50;
const NO_PROGRESS_WINDOW = 3;
const NO_PROGRESS_METRES = 10;
const LINEAR_HORIZON_S = 4;
const HOLD_AFTER_S = 12;
const DECAY_TAU_S = 3;

const finite = (value: number | null): value is number =>
  value !== null && Number.isFinite(value);

const stableCoordinate = (value: number): number =>
  Math.round(value * 1_000_000) / 1_000_000;

const distanceMetres = (
  fromXcm: number,
  fromYcm: number,
  toXcm: number,
  toYcm: number,
): number => Math.hypot(toXcm - fromXcm, toYcm - fromYcm) / 100;

/** Gateway axes: decreasing game X is north; increasing game Y is east. */
const bearingTo = (
  fromXcm: number,
  fromYcm: number,
  toXcm: number,
  toYcm: number,
): number => normalizeDeg(
  (Math.atan2(toYcm - fromYcm, -(toXcm - fromXcm)) * 180) / Math.PI,
);

export const normalizeDeg = (value: number): number =>
  ((value % 360) + 360) % 360;

/** Signed shortest turn from one bearing to another. */
export function shortestDeltaDeg(fromDeg: number, toDeg: number): number {
  return ((toDeg - fromDeg + 540) % 360) - 180;
}

/** Move a displayed angle over the shortest arc without jitter or full spins. */
export function advanceAngleDeg(
  currentDeg: number,
  targetDeg: number,
  elapsedS: number,
  maxRateDegS = 120,
  deadbandDeg = 4,
): number {
  const delta = shortestDeltaDeg(currentDeg, targetDeg);
  if (Math.abs(delta) <= deadbandDeg) return normalizeDeg(currentDeg);
  const step = Math.sign(delta)
    * Math.min(Math.abs(delta), maxRateDegS * Math.max(0, elapsedS));
  return normalizeDeg(currentDeg + step);
}

/**
 * Effective travel time used for local presentation. Motion is linear for the
 * first window, then exponentially decays until the honest hold boundary.
 */
export function effectiveProjectionAgeS(
  ageS: number,
  linearHorizonS = 4,
  holdAfterS = 12,
  decayTauS = 3,
): number {
  const boundedAge = Math.max(0, Math.min(ageS, holdAfterS));
  if (boundedAge <= linearHorizonS) return boundedAge;
  return linearHorizonS
    + decayTauS * (1 - Math.exp(-(boundedAge - linearHorizonS) / decayTauS));
}

export const freshnessForAge = (ageS: number): NavigationFreshness =>
  ageS <= 6 ? "tracking" : ageS <= 12 ? "estimating" : "waiting";

function smoothstep(value: number): number {
  const bounded = Math.max(0, Math.min(1, value));
  return bounded * bounded * (3 - 2 * bounded);
}

function baseManeuver(deltaDeg: number): NavigationManeuver {
  const magnitude = Math.abs(deltaDeg);
  if (magnitude <= 12) return "straight";
  if (magnitude <= 35) return deltaDeg < 0 ? "slight-left" : "slight-right";
  if (magnitude <= 110) return deltaDeg < 0 ? "left" : "right";
  return "turn-back";
}

/** Retain the current instruction inside a four-degree boundary band. */
function maneuverWithDeadband(
  deltaDeg: number,
  current: NavigationManeuver | null,
): NavigationManeuver {
  if (!current || current === "arrived" || current === "hold-cardinal") {
    return baseManeuver(deltaDeg);
  }

  const magnitude = Math.abs(deltaDeg);
  const sameSide =
    current === "straight"
    || current === "turn-back"
    || (deltaDeg < 0 && current.endsWith("left"))
    || (deltaDeg >= 0 && current.endsWith("right"));
  if (!sameSide) return baseManeuver(deltaDeg);

  if (current === "straight" && magnitude <= 16) return current;
  if ((current === "slight-left" || current === "slight-right")
      && magnitude >= 8 && magnitude <= 39) return current;
  if ((current === "left" || current === "right")
      && magnitude >= 31 && magnitude <= 114) return current;
  if (current === "turn-back" && magnitude >= 106) return current;
  return baseManeuver(deltaDeg);
}

export class NavigationEstimator {
  private sample: ConfirmedNavigationSample | null = null;
  private target: EstimatorTarget | null = null;
  private arrivalRadiusM = DEFAULT_ARRIVAL_RADIUS_M;
  private arrivalLatched = false;
  private arrivalFromPrediction = false;
  private arrivalPosition: Coordinates | null = null;
  private latchedTargetBearingDeg: number | null = null;
  private displayedTargetBearingDeg: number | null = null;
  private targetBearingEvaluatedAtMs: number | null = null;
  private correctionFrom: Coordinates | null = null;
  private correctionStartedAtMs = 0;
  private correctionDurationMs = 0;
  private currentCourseSource: CourseSource | null = null;
  private pendingCourse: { source: CourseSource; sinceMs: number; angleDeg: number } | null = null;
  private courseTargetDeg: number | null = null;
  private courseAnchorDeg: number | null = null;
  private courseAnchorAtMs: number | null = null;
  private currentManeuver: NavigationManeuver | null = null;
  private pendingManeuver: { maneuver: NavigationManeuver; sinceMs: number } | null = null;
  private maneuverEvaluatedAtMs: number | null = null;
  private confirmedDistancesM: number[] = [];

  accept(sample: ConfirmedNavigationSample): void {
    if (this.sample
        && this.targetBearingEvaluatedAtMs !== null
        && sample.confirmedAtMs >= this.targetBearingEvaluatedAtMs) {
      this.advanceTargetBearingTo(sample.confirmedAtMs);
    }
    if (this.sample
        && this.maneuverEvaluatedAtMs !== null
        && sample.confirmedAtMs >= this.maneuverEvaluatedAtMs) {
      this.advanceManeuverTo(sample.confirmedAtMs);
    }
    const previous = this.sample
      ? this.positionAt(sample.confirmedAtMs)
      : null;
    const correctionM = previous
      ? distanceMetres(previous.xCm, previous.yCm, sample.xCm, sample.yCm)
      : 0;
    const reset = sample.relocated || correctionM > 100;

    const accepted = reset ? this.withoutVelocity(sample) : { ...sample };
    this.sample = accepted;
    if (!previous || reset) {
      this.correctionFrom = null;
      this.correctionDurationMs = 0;
    } else {
      this.correctionFrom = previous;
      this.correctionStartedAtMs = sample.confirmedAtMs;
      this.correctionDurationMs = correctionM < 30 ? 650 : 300;
    }

    if (this.arrivalLatched && this.target) {
      const confirmedDistanceM = distanceMetres(
        accepted.xCm,
        accepted.yCm,
        this.target.xCm,
        this.target.yCm,
      );
      if (confirmedDistanceM > this.arrivalRadiusM + ARRIVAL_EXIT_MARGIN_M) {
        this.clearArrivalLatch();
        this.resetManeuver();
      }
    }

    if (reset) this.resetGuidance();
    if (this.displayedTargetBearingDeg === null && this.target) {
      this.displayedTargetBearingDeg = this.rawTargetBearingAt(accepted.confirmedAtMs);
      this.targetBearingEvaluatedAtMs = accepted.confirmedAtMs;
    }
    this.updateCourseCandidate(accepted);
    this.recordConfirmedProgress(accepted);
    this.evaluateManeuverAt(accepted.confirmedAtMs);
    this.maneuverEvaluatedAtMs = accepted.confirmedAtMs;
  }

  setTarget(target: EstimatorTarget | null): void {
    const changed = this.target?.id !== target?.id
      || this.target?.xCm !== target?.xCm
      || this.target?.yCm !== target?.yCm;
    this.target = target ? { ...target } : null;
    if (changed) {
      this.confirmedDistancesM = [];
      this.clearArrivalLatch();
      this.resetTargetBearing();
      this.resetManeuver();
    }
  }

  setArrivalRadiusM(radiusM: number): void {
    const next = Number.isFinite(radiusM) && radiusM >= 0
      ? radiusM
      : DEFAULT_ARRIVAL_RADIUS_M;
    if (next === this.arrivalRadiusM) return;
    this.arrivalRadiusM = next;
    this.clearArrivalLatch();
    this.resetManeuver();
  }

  /** Stop extrapolation after a rejected/outlier server observation. */
  invalidatePrediction(): void {
    if (!this.sample) return;
    if (this.arrivalLatched && this.arrivalFromPrediction) {
      this.clearArrivalLatch();
      this.resetTargetBearing();
    }
    this.sample = this.withoutVelocity(this.sample);
    this.correctionFrom = null;
    this.correctionDurationMs = 0;
    this.resetCourse();
    this.updateCourseCandidate(this.sample);
    this.resetManeuver();
  }

  snapshot(nowMs: number): NavigationSnapshot | null {
    if (!this.sample) return null;
    const projectedPosition = this.projectedCoordinatesAt(nowMs);
    this.updateArrivalLatch(projectedPosition);
    const position = this.positionAt(nowMs);
    const ageS = Math.max(0, (nowMs - this.sample.confirmedAtMs) / 1_000);
    const hasVelocity = this.hasCompleteVelocity(this.sample);
    const targetDistanceM = this.target
      ? distanceMetres(position.xCm, position.yCm, this.target.xCm, this.target.yCm)
      : null;
    const arrived = this.arrivalLatched;
    const targetBearingDeg = arrived
      ? this.latchedTargetBearingDeg
      : this.advanceTargetBearingTo(nowMs);
    const guidanceCourseDeg = this.updateDisplayedCourse(nowMs);
    this.advanceManeuverTo(nowMs);
    const maneuver = this.currentManeuver ?? "hold-cardinal";

    return {
      ...position,
      targetBearingDeg,
      guidanceCourseDeg,
      targetDistanceM,
      maneuver,
      freshness: freshnessForAge(ageS),
      predicting: hasVelocity && !arrived && ageS > 0 && ageS <= HOLD_AFTER_S,
      arrived,
      noProgress: this.hasNoProgress(),
    };
  }

  private positionAt(nowMs: number): Coordinates {
    if (this.arrivalLatched && this.arrivalPosition) {
      return { ...this.arrivalPosition };
    }
    return this.rawPositionAt(nowMs);
  }

  private rawPositionAt(nowMs: number): Coordinates {
    const target = this.projectedCoordinatesAt(nowMs);
    if (!this.correctionFrom || this.correctionDurationMs <= 0) return target;

    const t = smoothstep(
      (nowMs - this.correctionStartedAtMs) / this.correctionDurationMs,
    );
    if (t >= 1) return target;
    const lerp = (from: number, to: number) => from + (to - from) * t;
    return {
      xCm: lerp(this.correctionFrom.xCm, target.xCm),
      yCm: lerp(this.correctionFrom.yCm, target.yCm),
      px: lerp(this.correctionFrom.px, target.px),
      py: lerp(this.correctionFrom.py, target.py),
    };
  }

  private projectedCoordinatesAt(nowMs: number): Coordinates {
    const sample = this.sample!;
    const ageS = Math.max(0, (nowMs - sample.confirmedAtMs) / 1_000);
    const travelS = effectiveProjectionAgeS(
      ageS,
      LINEAR_HORIZON_S,
      HOLD_AFTER_S,
      DECAY_TAU_S,
    );
    const hasVelocity = this.hasCompleteVelocity(sample);
    return {
      xCm: sample.xCm + (hasVelocity ? sample.velocityXCmS! * travelS : 0),
      yCm: sample.yCm + (hasVelocity ? sample.velocityYCmS! * travelS : 0),
      px: sample.px + (hasVelocity ? sample.velocityPxXS! * travelS : 0),
      py: sample.py + (hasVelocity ? sample.velocityPxYS! * travelS : 0),
    };
  }

  private hasCompleteVelocity(sample: ConfirmedNavigationSample): boolean {
    return finite(sample.velocityXCmS)
      && finite(sample.velocityYCmS)
      && finite(sample.velocityPxXS)
      && finite(sample.velocityPxYS);
  }

  private desiredCourse(sample: ConfirmedNavigationSample): {
    source: CourseSource;
    angleDeg: number;
  } | null {
    const moving = finite(sample.velocityXCmS)
      && finite(sample.velocityYCmS)
      && Math.hypot(sample.velocityXCmS, sample.velocityYCmS) >= 1;
    if (moving && finite(sample.motionCourseDeg)) {
      return { source: "motion", angleDeg: normalizeDeg(sample.motionCourseDeg) };
    }
    if (finite(sample.serverFacingDeg)) {
      return { source: "server", angleDeg: normalizeDeg(sample.serverFacingDeg) };
    }
    return null;
  }

  private updateCourseCandidate(sample: ConfirmedNavigationSample): void {
    const desired = this.desiredCourse(sample);
    if (!desired) {
      this.resetCourse();
      return;
    }
    if (desired.source === this.currentCourseSource) {
      const current = this.courseAt(sample.confirmedAtMs);
      if (current !== null) {
        this.courseAnchorDeg = current;
        this.courseAnchorAtMs = sample.confirmedAtMs;
      }
      this.courseTargetDeg = desired.angleDeg;
      this.pendingCourse = null;
      return;
    }
    if (this.pendingCourse?.source === desired.source) {
      this.pendingCourse.angleDeg = desired.angleDeg;
      return;
    }
    this.pendingCourse = {
      source: desired.source,
      sinceMs: sample.confirmedAtMs,
      angleDeg: desired.angleDeg,
    };
  }

  private updateDisplayedCourse(nowMs: number): number | null {
    if (this.sample
        && nowMs - this.sample.confirmedAtMs > HOLD_AFTER_S * 1_000) {
      return null;
    }
    if (this.pendingCourse
        && nowMs - this.pendingCourse.sinceMs >= COURSE_SOURCE_STABLE_MS) {
      const switchAtMs = this.pendingCourse.sinceMs + COURSE_SOURCE_STABLE_MS;
      const priorAtSwitch = this.courseAt(switchAtMs);
      this.currentCourseSource = this.pendingCourse.source;
      this.courseAnchorDeg = priorAtSwitch ?? this.pendingCourse.angleDeg;
      this.courseAnchorAtMs = switchAtMs;
      this.courseTargetDeg = this.pendingCourse.angleDeg;
      this.pendingCourse = null;
    }
    return this.courseAt(nowMs);
  }

  private courseAt(nowMs: number): number | null {
    if (this.courseAnchorDeg === null
        || this.courseAnchorAtMs === null
        || this.courseTargetDeg === null) {
      return null;
    }
    return advanceAngleDeg(
      this.courseAnchorDeg,
      this.courseTargetDeg,
      Math.max(0, (nowMs - this.courseAnchorAtMs) / 1_000),
      120,
      4,
    );
  }

  private evaluateManeuverAt(nowMs: number): void {
    const candidate = this.maneuverCandidateAt(nowMs);
    if (candidate === "arrived"
        || candidate === "hold-cardinal"
        || this.currentManeuver === null
        || this.currentManeuver === "hold-cardinal") {
      this.currentManeuver = candidate;
      this.pendingManeuver = null;
      return;
    }
    if (candidate === this.currentManeuver) {
      this.pendingManeuver = null;
      return;
    }
    if (this.pendingManeuver?.maneuver !== candidate) {
      this.pendingManeuver = { maneuver: candidate, sinceMs: nowMs };
      return;
    }
    if (nowMs - this.pendingManeuver.sinceMs >= MANEUVER_STABLE_MS) {
      this.currentManeuver = candidate;
      this.pendingManeuver = null;
    }
  }

  private advanceManeuverTo(nowMs: number): void {
    if (this.currentManeuver === "arrived" && this.arrivalLatched) {
      this.maneuverEvaluatedAtMs = nowMs;
      return;
    }
    if (this.maneuverEvaluatedAtMs === null || nowMs < this.maneuverEvaluatedAtMs) {
      this.evaluateManeuverAt(nowMs);
      this.maneuverEvaluatedAtMs = nowMs;
      return;
    }
    const activeUntilMs = this.sample
      ? this.sample.confirmedAtMs + HOLD_AFTER_S * 1_000
      : nowMs;
    const steppedUntilMs = Math.min(nowMs, activeUntilMs);
    let nextMs = (Math.floor(
      this.maneuverEvaluatedAtMs / MANEUVER_TIMELINE_STEP_MS,
    ) + 1) * MANEUVER_TIMELINE_STEP_MS;
    while (nextMs <= steppedUntilMs) {
      this.evaluateManeuverAt(nextMs);
      nextMs += MANEUVER_TIMELINE_STEP_MS;
    }
    this.maneuverEvaluatedAtMs = Math.max(
      this.maneuverEvaluatedAtMs,
      nextMs - MANEUVER_TIMELINE_STEP_MS,
    );
    if (nowMs > activeUntilMs
        && this.maneuverEvaluatedAtMs < activeUntilMs) {
      this.evaluateManeuverAt(activeUntilMs);
      this.maneuverEvaluatedAtMs = activeUntilMs;
    }
    if (nowMs > steppedUntilMs) {
      this.evaluateManeuverAt(nowMs);
      this.maneuverEvaluatedAtMs = nowMs;
    }
  }

  private maneuverCandidateAt(nowMs: number): NavigationManeuver {
    if (this.arrivalLatched) return "arrived";
    if (!this.target) return "hold-cardinal";
    const position = this.positionAt(nowMs);
    const targetBearingDeg = bearingTo(
      position.xCm,
      position.yCm,
      this.target.xCm,
      this.target.yCm,
    );
    const guidanceCourseDeg = this.updateDisplayedCourse(nowMs);
    if (guidanceCourseDeg === null) return "hold-cardinal";
    return maneuverWithDeadband(
      shortestDeltaDeg(guidanceCourseDeg, targetBearingDeg),
      this.currentManeuver,
    );
  }

  private recordConfirmedProgress(sample: ConfirmedNavigationSample): void {
    if (!this.target) return;
    this.confirmedDistancesM.push(distanceMetres(
      sample.xCm,
      sample.yCm,
      this.target.xCm,
      this.target.yCm,
    ));
    if (this.confirmedDistancesM.length > NO_PROGRESS_WINDOW) {
      this.confirmedDistancesM.shift();
    }
  }

  private hasNoProgress(): boolean {
    if (this.confirmedDistancesM.length < NO_PROGRESS_WINDOW) return false;
    return this.confirmedDistancesM[0]
      - this.confirmedDistancesM[this.confirmedDistancesM.length - 1]
      < NO_PROGRESS_METRES;
  }

  private resetGuidance(): void {
    this.resetCourse();
    this.resetTargetBearing();
    this.resetManeuver();
    this.confirmedDistancesM = [];
  }

  private resetCourse(): void {
    this.currentCourseSource = null;
    this.pendingCourse = null;
    this.courseTargetDeg = null;
    this.courseAnchorDeg = null;
    this.courseAnchorAtMs = null;
  }

  private resetManeuver(): void {
    this.currentManeuver = null;
    this.pendingManeuver = null;
    this.maneuverEvaluatedAtMs = null;
  }

  private resetTargetBearing(): void {
    this.displayedTargetBearingDeg = null;
    this.targetBearingEvaluatedAtMs = null;
  }

  private rawTargetBearingAt(nowMs: number): number | null {
    if (!this.sample || !this.target) return null;
    const position = this.positionAt(nowMs);
    return bearingTo(
      position.xCm,
      position.yCm,
      this.target.xCm,
      this.target.yCm,
    );
  }

  private advanceTargetBearingTo(nowMs: number): number | null {
    if (!this.sample || !this.target) {
      this.resetTargetBearing();
      return null;
    }
    const rawNow = this.rawTargetBearingAt(nowMs)!;
    if (this.displayedTargetBearingDeg === null
        || this.targetBearingEvaluatedAtMs === null
        || nowMs < this.targetBearingEvaluatedAtMs) {
      this.displayedTargetBearingDeg = rawNow;
      this.targetBearingEvaluatedAtMs = nowMs;
      return rawNow;
    }

    const activeUntilMs = this.sample.confirmedAtMs + HOLD_AFTER_S * 1_000;
    const steppedUntilMs = Math.min(nowMs, activeUntilMs);
    let nextMs = (Math.floor(
      this.targetBearingEvaluatedAtMs / MANEUVER_TIMELINE_STEP_MS,
    ) + 1) * MANEUVER_TIMELINE_STEP_MS;
    while (nextMs <= steppedUntilMs) {
      const target = this.rawTargetBearingAt(nextMs)!;
      this.displayedTargetBearingDeg = advanceAngleDeg(
        this.displayedTargetBearingDeg,
        target,
        (nextMs - this.targetBearingEvaluatedAtMs) / 1_000,
      );
      this.targetBearingEvaluatedAtMs = nextMs;
      nextMs += MANEUVER_TIMELINE_STEP_MS;
    }
    if (nowMs > activeUntilMs
        && this.targetBearingEvaluatedAtMs < activeUntilMs) {
      const target = this.rawTargetBearingAt(activeUntilMs)!;
      this.displayedTargetBearingDeg = advanceAngleDeg(
        this.displayedTargetBearingDeg,
        target,
        (activeUntilMs - this.targetBearingEvaluatedAtMs) / 1_000,
      );
      this.targetBearingEvaluatedAtMs = activeUntilMs;
    }
    if (nowMs > steppedUntilMs) {
      this.displayedTargetBearingDeg = advanceAngleDeg(
        this.displayedTargetBearingDeg,
        rawNow,
        (nowMs - this.targetBearingEvaluatedAtMs) / 1_000,
      );
      this.targetBearingEvaluatedAtMs = nowMs;
    }
    return this.displayedTargetBearingDeg;
  }

  private withoutVelocity(
    sample: ConfirmedNavigationSample,
  ): ConfirmedNavigationSample {
    return {
      ...sample,
      velocityXCmS: null,
      velocityYCmS: null,
      velocityPxXS: null,
      velocityPxYS: null,
      motionCourseDeg: null,
    };
  }

  private clearArrivalLatch(): void {
    this.arrivalLatched = false;
    this.arrivalFromPrediction = false;
    this.arrivalPosition = null;
    this.latchedTargetBearingDeg = null;
  }

  private updateArrivalLatch(position: Coordinates): void {
    if (this.arrivalLatched
        || !this.sample
        || !this.target) return;
    const start = this.sample;
    const dx = position.xCm - start.xCm;
    const dy = position.yCm - start.yCm;
    const radiusCm = this.arrivalRadiusM * 100;
    const fromTargetX = start.xCm - this.target.xCm;
    const fromTargetY = start.yCm - this.target.yCm;
    const c = fromTargetX * fromTargetX
      + fromTargetY * fromTargetY
      - radiusCm * radiusCm;
    let entryT: number | null = c <= 0 ? 0 : null;
    const a = dx * dx + dy * dy;
    if (entryT === null && a > Number.EPSILON) {
      const b = 2 * (fromTargetX * dx + fromTargetY * dy);
      const discriminant = b * b - 4 * a * c;
      if (discriminant >= 0) {
        const candidate = (-b - Math.sqrt(discriminant)) / (2 * a);
        if (candidate >= 0 && candidate <= 1) entryT = candidate;
      }
    }
    if (entryT === null) return;

    const entry: Coordinates = {
      xCm: stableCoordinate(start.xCm + dx * entryT),
      yCm: stableCoordinate(start.yCm + dy * entryT),
      px: stableCoordinate(start.px + (position.px - start.px) * entryT),
      py: stableCoordinate(start.py + (position.py - start.py) * entryT),
    };

    this.arrivalLatched = true;
    this.arrivalFromPrediction = entryT > Number.EPSILON;
    this.arrivalPosition = entry;
    this.latchedTargetBearingDeg = bearingTo(
      start.xCm,
      start.yCm,
      this.target.xCm,
      this.target.yCm,
    );
    this.currentManeuver = "arrived";
    this.pendingManeuver = null;
  }
}
