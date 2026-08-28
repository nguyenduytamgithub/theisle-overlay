import assert from "node:assert/strict";
import test from "node:test";

import {
  advanceAngleDeg,
  effectiveProjectionAgeS,
  freshnessForAge,
  shortestDeltaDeg,
} from "./estimator.ts";

test("359 to 1 uses the two-degree short arc", () => {
  assert.equal(shortestDeltaDeg(359, 1), 2);
  assert.equal(shortestDeltaDeg(1, 359), -2);
});

test("visual angle obeys rate limit and crosses north without spinning", () => {
  assert.equal(advanceAngleDeg(350, 10, 0.05, 120, 4), 356);
  assert.equal(advanceAngleDeg(359, 1, 1, 120, 4), 359);
});

test("projection decays and is fully held after twelve seconds", () => {
  assert.equal(effectiveProjectionAgeS(4, 4, 12, 3), 4);
  assert.ok(Math.abs(effectiveProjectionAgeS(16, 4, 12, 3) - 6.79155) < 0.0001);
});

test("freshness labels are honest at boundaries", () => {
  assert.equal(freshnessForAge(6), "tracking");
  assert.equal(freshnessForAge(12), "estimating");
  assert.equal(freshnessForAge(12.001), "waiting");
});
