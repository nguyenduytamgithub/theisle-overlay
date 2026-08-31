import assert from "node:assert/strict";
import test from "node:test";

import {
  instructionFor,
  nextAlignmentLocked,
  projectToSegment,
  steeringPromptFor,
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

test("the center ray stays vertical for left and right turns", () => {
  const east = waterGuideFrame(
    route({ targetXCm: 0, targetYCm: 100_000 }),
    view({ guidanceCourseDeg: 0 }),
  );
  const west = waterGuideFrame(
    route({ targetXCm: 0, targetYCm: -100_000 }),
    view({ guidanceCourseDeg: 0 }),
  );

  assert.equal(east.turn, "right");
  assert.equal(east.screenAngleDeg, 0);
  assert.equal(west.turn, "left");
  assert.equal(west.screenAngleDeg, 0);
});

test("alignment enters at eight degrees and holds until beyond eighteen", () => {
  const eight = waterGuideFrame(route(), view({ guidanceCourseDeg: 352 }));
  const nine = waterGuideFrame(route(), view({ guidanceCourseDeg: 351 }));
  const eighteen = waterGuideFrame(route(), view({ guidanceCourseDeg: 342 }));
  const beyond = waterGuideFrame(route(), view({ guidanceCourseDeg: 341.9 }));

  assert.equal(nextAlignmentLocked(false, eight), true);
  assert.equal(nextAlignmentLocked(false, nine), false);
  assert.equal(nextAlignmentLocked(true, eighteen), true);
  assert.equal(nextAlignmentLocked(true, beyond), false);
});

test("steering prompt makes the stop point and turn direction explicit", () => {
  const aligned = waterGuideFrame(route(), view({ guidanceCourseDeg: 355 }));
  const right = waterGuideFrame(route(), view({ guidanceCourseDeg: 325 }));
  const left = waterGuideFrame(route(), view({ guidanceCourseDeg: 35 }));

  assert.equal(
    steeringPromptFor(aligned, true, "vi"),
    "✓ ĐÚNG HƯỚNG · GIỮ W",
  );
  assert.equal(
    steeringPromptFor(right, false, "vi"),
    "XOAY NHÂN VẬT PHẢI 35° →",
  );
  assert.equal(
    steeringPromptFor(left, false, "vi"),
    "← XOAY NHÂN VẬT TRÁI 35°",
  );
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

test("Vietnamese copy prioritizes U-turn and explains recovery states", () => {
  const onRoute = waterGuideFrame(route(), view());
  const uturn = waterGuideFrame(route(), view({ guidanceCourseDeg: 180 }));
  const offRoute = waterGuideFrame(route(), view({ yCm: 2_000 }));
  const stale = waterGuideFrame(route(), view({ freshness: "waiting" }));

  assert.equal(
    instructionFor(onRoute, "vi"),
    "TIA CỐ ĐỊNH · LÀM THEO MŨI TÊN",
  );
  assert.equal(instructionFor(uturn, "vi"), "QUAY ĐẦU");
  assert.equal(
    instructionFor(offRoute, "vi"),
    "LỆCH ĐƯỜNG · LÀM THEO MŨI TÊN",
  );
  assert.equal(instructionFor(stale, "vi"), "CHỜ SERVER");
});
