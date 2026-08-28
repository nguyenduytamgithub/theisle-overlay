import assert from "node:assert/strict";
import test from "node:test";

import {
  compassPoint,
  compassTapeTicks,
  relativeBearing,
} from "./guidance.ts";

test("relative bearing chooses the shortest signed turn", () => {
  assert.equal(relativeBearing(350, 10), 20);
  assert.equal(relativeBearing(10, 350), -20);
  assert.equal(relativeBearing(90, 270), -180);
});

test("Vietnamese cardinal and intercardinal names are stable", () => {
  assert.equal(compassPoint(0, "vi"), "BẮC");
  assert.equal(compassPoint(90, "vi"), "ĐÔNG");
  assert.equal(compassPoint(180, "vi"), "NAM");
  assert.equal(compassPoint(270, "vi"), "TÂY");
  assert.equal(compassPoint(315, "vi"), "TÂY BẮC");
});

test("compass tape spans both sides of north without a discontinuity", () => {
  const ticks = compassTapeTicks(355, 50, 5);
  assert.deepEqual(ticks.map((tick) => tick.bearing), [305, 330, 355, 20, 45]);
  assert.deepEqual(ticks.map((tick) => tick.offsetDeg), [-50, -25, 0, 25, 50]);
});
