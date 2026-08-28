import assert from "node:assert/strict";
import test from "node:test";

import { projectedPosition, smoothedPosition } from "./prediction.ts";

const sample = {
  xCm: 10_000,
  yCm: 20_000,
  px: 100,
  py: 200,
  velocityXCmS: 300,
  velocityYCmS: -100,
  velocityPxXS: 3,
  velocityPxYS: -1,
  confirmedAtMs: 1_000,
  predictionHorizonS: 4,
  staleAfterS: 12,
};

test("projects continuously from the confirmed sample", () => {
  const out = projectedPosition(sample, 3_000);
  assert.deepEqual(
    { xCm: out.xCm, yCm: out.yCm, px: out.px, py: out.py },
    { xCm: 10_600, yCm: 19_800, px: 106, py: 198 },
  );
  assert.equal(out.predicting, true);
  assert.equal(out.stale, false);
});

test("freezes at the bounded horizon and later becomes stale", () => {
  const frozen = projectedPosition(sample, 8_000);
  assert.equal(frozen.px, 112);
  assert.equal(frozen.py, 196);
  assert.equal(frozen.predicting, false);
  assert.equal(frozen.stale, false);

  const stale = projectedPosition(sample, 14_000);
  assert.equal(stale.px, 112);
  assert.equal(stale.stale, true);
});

test("missing velocity never invents movement", () => {
  const out = projectedPosition(
    { ...sample, velocityXCmS: null, velocityYCmS: null, velocityPxXS: null, velocityPxYS: null },
    3_000,
  );
  assert.equal(out.px, sample.px);
  assert.equal(out.py, sample.py);
  assert.equal(out.predicting, false);
});

test("server correction blends from the last displayed position", () => {
  const target = projectedPosition(sample, 1_200);
  const from = { xCm: 9_000, yCm: 19_000, px: 90, py: 190 };

  const halfway = smoothedPosition(target, from, 1_000, 1_175, 350);
  assert.ok(halfway.px > from.px && halfway.px < target.px);
  assert.ok(halfway.py > from.py && halfway.py < target.py);

  const done = smoothedPosition(target, from, 1_000, 1_400, 350);
  assert.deepEqual(done, target);
});
