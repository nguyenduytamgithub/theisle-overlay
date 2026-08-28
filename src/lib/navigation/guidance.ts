export type HudLanguage = "vi" | "en";

const POINTS = {
  vi: ["BẮC", "ĐÔNG BẮC", "ĐÔNG", "ĐÔNG NAM", "NAM", "TÂY NAM", "TÂY", "TÂY BẮC"],
  en: ["N", "NE", "E", "SE", "S", "SW", "W", "NW"],
} as const;

export const normalizeBearing = (bearing: number) =>
  ((bearing % 360) + 360) % 360;

/** Signed shortest turn from current heading to target: left < 0, right > 0. */
export function relativeBearing(headingDeg: number, targetBearingDeg: number): number {
  return ((targetBearingDeg - headingDeg + 540) % 360) - 180;
}

export function compassPoint(bearingDeg: number, language: HudLanguage): string {
  const index = Math.round(normalizeBearing(bearingDeg) / 45) % 8;
  return POINTS[language][index];
}

export interface CompassTapeTick {
  bearing: number;
  offsetDeg: number;
  label: string;
  cardinal: boolean;
}

export function compassTapeTicks(
  headingDeg: number,
  halfSpanDeg: number = 100,
  count: number = 9,
  language: HudLanguage = "vi",
): CompassTapeTick[] {
  const safeCount = Math.max(1, Math.floor(count));
  const step = safeCount === 1 ? 0 : (halfSpanDeg * 2) / (safeCount - 1);
  return Array.from({ length: safeCount }, (_, index) => {
    const offsetDeg = -halfSpanDeg + step * index;
    const bearing = normalizeBearing(headingDeg + offsetDeg);
    const nearest45 = Math.round(bearing / 45) * 45;
    const cardinal = Math.round(normalizeBearing(nearest45) / 45) % 2 === 0;
    return {
      bearing,
      offsetDeg,
      label: compassPoint(bearing, language),
      cardinal,
    };
  });
}

/** Gateway axes: decreasing game X is north; increasing game Y is east. */
export function bearingTo(
  fromXcm: number,
  fromYcm: number,
  toXcm: number,
  toYcm: number,
): number {
  return normalizeBearing(
    (Math.atan2(toYcm - fromYcm, -(toXcm - fromXcm)) * 180) / Math.PI,
  );
}

export function distanceMetres(
  fromXcm: number,
  fromYcm: number,
  toXcm: number,
  toYcm: number,
): number {
  return Math.hypot(toXcm - fromXcm, toYcm - fromYcm) / 100;
}
