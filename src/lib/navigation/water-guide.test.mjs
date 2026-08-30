import assert from "node:assert/strict";
import test from "node:test";

import {
  projectToSegment,
  waterGuideFrame,
} from "./water-guide.ts";

const route = (overrides = {}) => ({
  startXCm: 0,
  startYCm: 0,
  targetXCm: -100_000,
  targetYCm: 0,
  targetMaskPx: [100, 200],
  label: "North Lake",
  initialDistanceM: 1_000,
  ...overrides,
});

const view = (overrides = {}) => ({
  xCm: 0,
  yCm: 0,
  guidanceCourseDeg: 0,
  freshness: "tracking",
  ...overrides,
});

test("projection clamps to the fixed segment endpoints", () => {
  assert.deepEqual(
    projectToSegment([-5_000, 0], [0, 0], [10_000, 0]),
    { pointCm: [0, 0], t: 0, crossTrackM: 50 },
  );
  assert.deepEqual(
    projectToSegment([15_000, 0], [0, 0], [10_000, 0]),
    { pointCm: [10_000, 0], t: 1, crossTrackM: 50 },
  );
});

test("off-route player is guided eighty metres ahead on the original line", () => {
  const frame = waterGuideFrame(
    route({ startXCm: 0, startYCm: 0, targetXCm: 100_000, targetYCm: 0 }),
    view({ xCm: 20_000, yCm: 10_000 }),
  );

  assert.equal(Math.round(frame.crossTrackM), 100);
  assert.deepEqual(frame.steeringTargetCm, [28_000, 0]);
  assert.equal(frame.state, "off-route");
});

test("fifteen metres is on-route while just beyond is off-route", () => {
  const exactly = waterGuideFrame(route(), view({ yCm: 1_500 }));
  const beyond = waterGuideFrame(route(), view({ yCm: 1_501 }));

  assert.equal(exactly.state, "on-route");
  assert.equal(beyond.state, "off-route");
});

test("one-hundred-fifty metres is off-route while just beyond is lost", () => {
  const exactly = waterGuideFrame(route(), view({ yCm: 15_000 }));
  const beyond = waterGuideFrame(route(), view({ yCm: 15_001 }));

  assert.equal(exactly.state, "off-route");
  assert.equal(beyond.state, "lost");
});

test("twenty-five-metre arrival hides the ray", () => {
  const frame = waterGuideFrame(route(), view({ xCm: -97_500 }));

  assert.equal(frame.state, "arrived");
  assert.equal(frame.remainingM, 25);
  assert.equal(frame.rayVisible, false);
});

test("one-hundred-eighty-degree error requests a U-turn", () => {
  const frame = waterGuideFrame(route(), view({ guidanceCourseDeg: 180 }));

  assert.equal(frame.turn, "uturn");
  assert.equal(Math.abs(frame.relativeDeg), 180);
  assert.equal(frame.rayVisible, true);
});

test("shortest turn crosses north without spinning", () => {
  const frame = waterGuideFrame(route(), view({ guidanceCourseDeg: 350 }));

  assert.equal(frame.relativeDeg, 10);
  assert.equal(frame.turn, "straight");
});

test("left and right signs point the screen ray away from its origin", () => {
  const east = waterGuideFrame(
    route({ targetXCm: 0, targetYCm: 100_000 }),
    view({ guidanceCourseDeg: 0 }),
  );
  const west = waterGuideFrame(
    route({ targetXCm: 0, targetYCm: -100_000 }),
    view({ guidanceCourseDeg: 0 }),
  );

  assert.equal(east.turn, "right");
  assert.equal(east.screenAngleDeg, 75);
  assert.equal(west.turn, "left");
  assert.equal(west.screenAngleDeg, -75);
});

test("stale or headingless evidence never emits a confident ray", () => {
  const stale = waterGuideFrame(route(), view({ freshness: "waiting" }));
  const headingless = waterGuideFrame(route(), view({ guidanceCourseDeg: null }));

  assert.equal(stale.state, "waiting");
  assert.equal(stale.rayVisible, false);
  assert.equal(headingless.state, "heading-unknown");
  assert.equal(headingless.rayVisible, false);
});

test("zero-length or non-finite route fails closed", () => {
  const zero = waterGuideFrame(
    route({ startXCm: 10, startYCm: 20, targetXCm: 10, targetYCm: 20 }),
    view(),
  );
  const invalid = waterGuideFrame(route({ targetXCm: Number.NaN }), view());

  assert.equal(zero.state, "invalid");
  assert.equal(zero.rayVisible, false);
  assert.equal(invalid.state, "invalid");
  assert.equal(invalid.rayVisible, false);
});
