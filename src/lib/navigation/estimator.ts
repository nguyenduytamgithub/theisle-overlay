export type NavigationFreshness = "tracking" | "estimating" | "waiting";

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
