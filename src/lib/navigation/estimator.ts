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

const ARRIVAL_RADIUS_M = 25;
const COURSE_SOURCE_STABLE_MS = 1_000;
const MANEUVER_STABLE_MS = 600;
const NO_PROGRESS_WINDOW = 3;
const NO_PROGRESS_METRES = 10;
const LINEAR_HORIZON_S = 4;
const HOLD_AFTER_S = 12;
const DECAY_TAU_S = 3;

const finite = (value: number | null): value is number =>
  value !== null && Number.isFinite(value);

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
  private confirmedDistancesM: number[] = [];

  accept(sample: ConfirmedNavigationSample): void {
    const previous = this.sample
      ? this.positionAt(sample.confirmedAtMs)
      : null;
    const correctionM = previous
      ? distanceMetres(previous.xCm, previous.yCm, sample.xCm, sample.yCm)
      : 0;
    const reset = sample.relocated || correctionM > 100;

    this.sample = { ...sample };
    if (!previous || reset) {
      this.correctionFrom = null;
      this.correctionDurationMs = 0;
    } else {
      this.correctionFrom = previous;
      this.correctionStartedAtMs = sample.confirmedAtMs;
      this.correctionDurationMs = correctionM < 30 ? 650 : 300;
    }

    if (reset) this.resetGuidance();
    this.updateCourseCandidate(sample);
    this.recordConfirmedProgress(sample);
  }

  setTarget(target: EstimatorTarget | null): void {
    const changed = this.target?.id !== target?.id
      || this.target?.xCm !== target?.xCm
      || this.target?.yCm !== target?.yCm;
    this.target = target ? { ...target } : null;
    if (changed) {
      this.confirmedDistancesM = [];
      this.currentManeuver = null;
      this.pendingManeuver = null;
    }
  }

  snapshot(nowMs: number): NavigationSnapshot | null {
    if (!this.sample) return null;
    const position = this.positionAt(nowMs);
    const ageS = Math.max(0, (nowMs - this.sample.confirmedAtMs) / 1_000);
    const hasVelocity = this.hasCompleteVelocity(this.sample);
    const targetDistanceM = this.target
      ? distanceMetres(position.xCm, position.yCm, this.target.xCm, this.target.yCm)
      : null;
    const arrived = targetDistanceM !== null && targetDistanceM <= ARRIVAL_RADIUS_M;
    const targetBearingDeg = this.target
      ? bearingTo(position.xCm, position.yCm, this.target.xCm, this.target.yCm)
      : null;
    const guidanceCourseDeg = this.updateDisplayedCourse(nowMs);

    let candidate: NavigationManeuver;
    if (arrived) {
      candidate = "arrived";
    } else if (targetBearingDeg === null || guidanceCourseDeg === null) {
      candidate = "hold-cardinal";
    } else {
      candidate = maneuverWithDeadband(
        shortestDeltaDeg(guidanceCourseDeg, targetBearingDeg),
        this.currentManeuver,
      );
    }
    const maneuver = this.updateManeuver(candidate, nowMs, arrived);

    return {
      ...position,
      targetBearingDeg,
      guidanceCourseDeg,
      targetDistanceM,
      maneuver,
      freshness: freshnessForAge(ageS),
      predicting: hasVelocity && ageS > 0 && ageS <= HOLD_AFTER_S,
      arrived,
      noProgress: this.hasNoProgress(),
    };
  }

  private positionAt(nowMs: number): Coordinates {
    const sample = this.sample!;
    const ageS = Math.max(0, (nowMs - sample.confirmedAtMs) / 1_000);
    const travelS = effectiveProjectionAgeS(
      ageS,
      LINEAR_HORIZON_S,
      HOLD_AFTER_S,
      DECAY_TAU_S,
    );
    const hasVelocity = this.hasCompleteVelocity(sample);
    const target: Coordinates = {
      xCm: sample.xCm + (hasVelocity ? sample.velocityXCmS! * travelS : 0),
      yCm: sample.yCm + (hasVelocity ? sample.velocityYCmS! * travelS : 0),
      px: sample.px + (hasVelocity ? sample.velocityPxXS! * travelS : 0),
      py: sample.py + (hasVelocity ? sample.velocityPxYS! * travelS : 0),
    };
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
      this.pendingCourse = null;
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

  private updateManeuver(
    candidate: NavigationManeuver,
    nowMs: number,
    arrived: boolean,
  ): NavigationManeuver {
    if (arrived || this.currentManeuver === null) {
      this.currentManeuver = candidate;
      this.pendingManeuver = null;
      return candidate;
    }
    if (candidate === this.currentManeuver) {
      this.pendingManeuver = null;
      return this.currentManeuver;
    }
    if (this.pendingManeuver?.maneuver !== candidate) {
      this.pendingManeuver = { maneuver: candidate, sinceMs: nowMs };
      return this.currentManeuver;
    }
    if (nowMs - this.pendingManeuver.sinceMs >= MANEUVER_STABLE_MS) {
      this.currentManeuver = candidate;
      this.pendingManeuver = null;
    }
    return this.currentManeuver;
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
    this.currentCourseSource = null;
    this.pendingCourse = null;
    this.courseTargetDeg = null;
    this.courseAnchorDeg = null;
    this.courseAnchorAtMs = null;
    this.currentManeuver = null;
    this.pendingManeuver = null;
    this.confirmedDistancesM = [];
  }
}
