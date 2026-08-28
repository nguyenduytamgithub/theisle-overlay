export interface PredictionInput {
  xCm: number;
  yCm: number;
  px: number;
  py: number;
  velocityXCmS: number | null;
  velocityYCmS: number | null;
  velocityPxXS: number | null;
  velocityPxYS: number | null;
  confirmedAtMs: number;
  predictionHorizonS: number;
  staleAfterS: number;
}

export interface ProjectedPosition {
  xCm: number;
  yCm: number;
  px: number;
  py: number;
  ageS: number;
  predicting: boolean;
  stale: boolean;
}

type Coordinates = Pick<ProjectedPosition, "xCm" | "yCm" | "px" | "py">;

const finiteNumber = (value: number | null): value is number =>
  value !== null && Number.isFinite(value);

/**
 * Extrapolate only inside the backend-provided safety window. The confirmed
 * sample remains immutable; callers use this transient position for paint
 * only, never for trail persistence or waypoint geometry.
 */
export function projectedPosition(
  sample: PredictionInput,
  nowMs: number = Date.now(),
): ProjectedPosition {
  const ageS = Math.max(0, (nowMs - sample.confirmedAtMs) / 1_000);
  const horizonS = Math.max(0, sample.predictionHorizonS);
  const projectedAgeS = Math.min(ageS, horizonS);
  const hasVelocity =
    finiteNumber(sample.velocityXCmS) &&
    finiteNumber(sample.velocityYCmS) &&
    finiteNumber(sample.velocityPxXS) &&
    finiteNumber(sample.velocityPxYS);

  return {
    xCm: sample.xCm + (hasVelocity ? sample.velocityXCmS! * projectedAgeS : 0),
    yCm: sample.yCm + (hasVelocity ? sample.velocityYCmS! * projectedAgeS : 0),
    px: sample.px + (hasVelocity ? sample.velocityPxXS! * projectedAgeS : 0),
    py: sample.py + (hasVelocity ? sample.velocityPxYS! * projectedAgeS : 0),
    ageS,
    predicting: hasVelocity && ageS > 0 && ageS < horizonS,
    stale: ageS > Math.max(horizonS, sample.staleAfterS),
  };
}

/** Ease a newly confirmed server correction over a few frames. */
export function smoothedPosition(
  target: ProjectedPosition,
  from: Coordinates | null,
  correctionStartedAtMs: number,
  nowMs: number = Date.now(),
  correctionDurationMs: number = 350,
): ProjectedPosition {
  if (!from || correctionDurationMs <= 0) return target;
  const linear = Math.max(
    0,
    Math.min(1, (nowMs - correctionStartedAtMs) / correctionDurationMs),
  );
  if (linear >= 1) return target;
  const t = linear * linear * (3 - 2 * linear);
  const lerp = (a: number, b: number) => a + (b - a) * t;
  return {
    ...target,
    xCm: lerp(from.xCm, target.xCm),
    yCm: lerp(from.yCm, target.yCm),
    px: lerp(from.px, target.px),
    py: lerp(from.py, target.py),
  };
}
