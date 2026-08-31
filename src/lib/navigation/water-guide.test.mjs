import assert from "node:assert/strict";
import test from "node:test";

import {
  instructionFor,
  movementCourseBetween,
  nextAlignmentLocked,
  projectToSegment,
  steeringPromptFor,
  waterGuideBoardNeedles,
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
  movementCourseDeg: 0,
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

test("every XY update draws directly from the current position to the locked target", () => {
  const frame = waterGuideFrame(
    route({ startXCm: 0, startYCm: 0, targetXCm: 100_000, targetYCm: 0 }),
    view({ xCm: 20_000, yCm: 10_000 }),
  );

  assert.equal(frame.crossTrackM, 0);
  assert.deepEqual(frame.steeringTargetCm, [100_000, 0]);
  assert.equal(frame.state, "on-route");
});

test("deviation from the old route starts a new direct line instead of off-route", () => {
  const exactly = waterGuideFrame(route(), view({ yCm: 1_500 }));
  const beyond = waterGuideFrame(route(), view({ yCm: 1_501 }));

  assert.equal(exactly.state, "on-route");
  assert.equal(beyond.state, "on-route");
  assert.equal(exactly.crossTrackM, 0);
  assert.equal(beyond.crossTrackM, 0);
});

test("a changed XY point recalculates the direct target bearing", () => {
  const first = waterGuideFrame(route(), view({ xCm: 0, yCm: 15_000 }));
  const next = waterGuideFrame(route(), view({ xCm: -20_000, yCm: 15_000 }));

  assert.deepEqual(first.steeringTargetCm, [-100_000, 0]);
  assert.deepEqual(next.steeringTargetCm, [-100_000, 0]);
  assert.notEqual(first.desiredBearingDeg, next.desiredBearingDeg);
  assert.equal(first.state, "on-route");
  assert.equal(next.state, "on-route");
});

test("an obsolete activation start cannot block a later current-to-target ray", () => {
  const frame = waterGuideFrame(
    route({ startXCm: -100_000, startYCm: 0 }),
    view({ xCm: 0, yCm: 0 }),
  );

  assert.equal(frame.state, "on-route");
  assert.equal(frame.rayVisible, true);
  assert.equal(frame.remainingM, 1_000);
  assert.deepEqual(frame.steeringTargetCm, [-100_000, 0]);
});

test("twenty-five-metre arrival hides the ray", () => {
  const frame = waterGuideFrame(route(), view({ xCm: -97_500 }));

  assert.equal(frame.state, "arrived");
  assert.equal(frame.remainingM, 25);
  assert.equal(frame.rayVisible, false);
});

test("one-hundred-eighty-degree error requests a U-turn", () => {
  const frame = waterGuideFrame(route(), view({ movementCourseDeg: 180 }));

  assert.equal(frame.turn, "uturn");
  assert.equal(Math.abs(frame.relativeDeg), 180);
  assert.equal(frame.rayVisible, true);
});

test("shortest turn crosses north without spinning", () => {
  const frame = waterGuideFrame(route(), view({ movementCourseDeg: 350 }));

  assert.equal(frame.relativeDeg, 10);
  assert.equal(frame.turn, "straight");
});

test("the center ray stays vertical for left and right turns", () => {
  const east = waterGuideFrame(
    route({ targetXCm: 0, targetYCm: 100_000 }),
    view({ movementCourseDeg: 0 }),
  );
  const west = waterGuideFrame(
    route({ targetXCm: 0, targetYCm: -100_000 }),
    view({ movementCourseDeg: 0 }),
  );

  assert.equal(east.turn, "right");
  assert.equal(east.screenAngleDeg, 0);
  assert.equal(west.turn, "left");
  assert.equal(west.screenAngleDeg, 0);
});

test("alignment enters at eight degrees and holds until beyond eighteen", () => {
  const eight = waterGuideFrame(route(), view({ movementCourseDeg: 352 }));
  const nine = waterGuideFrame(route(), view({ movementCourseDeg: 351 }));
  const eighteen = waterGuideFrame(route(), view({ movementCourseDeg: 342 }));
  const beyond = waterGuideFrame(route(), view({ movementCourseDeg: 341.9 }));

  assert.equal(nextAlignmentLocked(false, eight), true);
  assert.equal(nextAlignmentLocked(false, nine), false);
  assert.equal(nextAlignmentLocked(true, eighteen), true);
  assert.equal(nextAlignmentLocked(true, beyond), false);
});

test("steering prompt makes the stop point and turn direction explicit", () => {
  const aligned = waterGuideFrame(route(), view({ movementCourseDeg: 355 }));
  const right = waterGuideFrame(route(), view({ movementCourseDeg: 325 }));
  const left = waterGuideFrame(route(), view({ movementCourseDeg: 35 }));

  assert.equal(
    steeringPromptFor(aligned, true, "vi"),
    "✓ ĐÚNG HƯỚNG · GIỮ W",
  );
  assert.equal(
    steeringPromptFor(right, false, "vi"),
    "QUỸ ĐẠO XY: RẼ PHẢI 35°",
  );
  assert.equal(
    steeringPromptFor(left, false, "vi"),
    "QUỸ ĐẠO XY: RẼ TRÁI 35°",
  );
});

test("movement course ignores sub-five-metre XY jitter", () => {
  assert.equal(
    movementCourseBetween({ xCm: 0, yCm: 0 }, { xCm: 0, yCm: 499 }),
    null,
  );
  assert.equal(
    movementCourseBetween({ xCm: 0, yCm: 0 }, { xCm: -500, yCm: 0 }),
    0,
  );
  assert.equal(
    movementCourseBetween({ xCm: 0, yCm: 0 }, { xCm: 0, yCm: 500 }),
    90,
  );
});

test("stale coordinates freeze the ray while missing movement stays fixed and honest", () => {
  const stale = waterGuideFrame(route(), view({ freshness: "waiting" }));
  const motionless = waterGuideFrame(route(), view({ movementCourseDeg: null }));

  assert.equal(stale.state, "waiting");
  assert.equal(stale.rayVisible, true);
  assert.equal(stale.turn, "none");
  assert.equal(stale.screenAngleDeg, 0);
  assert.equal(
    steeringPromptFor(stale, false, "vi"),
    "CHỜ TỌA ĐỘ MỚI · KIM GIỮ NGUYÊN",
  );
  assert.equal(motionless.state, "movement-unknown");
  assert.equal(motionless.rayVisible, true);
  assert.equal(motionless.screenAngleDeg, 0);
  assert.equal(motionless.turn, "none");
  assert.equal(nextAlignmentLocked(false, motionless), false);
  assert.equal(
    steeringPromptFor(motionless, false, "vi"),
    "ĐI ÍT NHẤT 5 M ĐỂ LẤY HƯỚNG",
  );
});

test("current position at target arrives while a non-finite target fails closed", () => {
  const zero = waterGuideFrame(
    route({ startXCm: 10, startYCm: 20, targetXCm: 10, targetYCm: 20 }),
    view(),
  );
  const invalid = waterGuideFrame(route({ targetXCm: Number.NaN }), view());

  assert.equal(zero.state, "arrived");
  assert.equal(zero.rayVisible, false);
  assert.equal(invalid.state, "invalid");
  assert.equal(invalid.rayVisible, false);
});

test("Vietnamese copy prioritizes U-turn and explains recovery states", () => {
  const onRoute = waterGuideFrame(route(), view());
  const uturn = waterGuideFrame(route(), view({ movementCourseDeg: 180 }));
  const redrawn = waterGuideFrame(route(), view({ yCm: 2_000 }));
  const stale = waterGuideFrame(route(), view({ freshness: "waiting" }));

  assert.equal(
    instructionFor(onRoute, "vi"),
    "BẢNG XY BẮC CỐ ĐỊNH · SO HAI KIM",
  );
  assert.equal(instructionFor(uturn, "vi"), "QUAY ĐẦU");
  assert.equal(
    instructionFor(redrawn, "vi"),
    "BẢNG XY BẮC CỐ ĐỊNH · SO HAI KIM",
  );
  assert.equal(
    instructionFor(stale, "vi"),
    "CHỜ TỌA ĐỘ MỚI · KIM GIỮ NGUYÊN",
  );
});

test("north-up board maps absolute XY bearings to fixed cardinal angles", () => {
  const north = waterGuideFrame(route(), view());
  const east = waterGuideFrame(
    route({ targetXCm: 0, targetYCm: 100_000 }),
    view(),
  );
  const south = waterGuideFrame(
    route({ targetXCm: 100_000, targetYCm: 0 }),
    view(),
  );
  const west = waterGuideFrame(
    route({ targetXCm: 0, targetYCm: -100_000 }),
    view(),
  );

  assert.equal(waterGuideBoardNeedles(north, 270).targetBearingDeg, 0);
  assert.equal(waterGuideBoardNeedles(east, 270).targetBearingDeg, 90);
  assert.equal(waterGuideBoardNeedles(south, 270).targetBearingDeg, 180);
  assert.equal(waterGuideBoardNeedles(west, 270).targetBearingDeg, 270);
  assert.equal(waterGuideBoardNeedles(east, 270).movementBearingDeg, 270);
});

test("north-up board hides only the movement needle before a course exists", () => {
  const frame = waterGuideFrame(route(), view({ movementCourseDeg: null }));
  const board = waterGuideBoardNeedles(frame, null);

  assert.equal(board.targetVisible, true);
  assert.equal(board.targetBearingDeg, 0);
  assert.equal(board.movementVisible, false);
  assert.equal(board.movementBearingDeg, null);
});

test("north-up board freezes known bearings while waiting for fresh XY", () => {
  const frame = waterGuideFrame(route(), view({ freshness: "waiting" }));
  const board = waterGuideBoardNeedles(frame, 90);

  assert.equal(frame.state, "waiting");
  assert.equal(board.targetVisible, true);
  assert.equal(board.targetBearingDeg, 0);
  assert.equal(board.movementVisible, true);
  assert.equal(board.movementBearingDeg, 90);
});

test("north-up board hides both needles after arrival or invalid data", () => {
  const arrived = waterGuideFrame(route(), view({ xCm: -97_500 }));
  const invalid = waterGuideFrame(route({ targetXCm: Number.NaN }), view());

  for (const frame of [arrived, invalid]) {
    const board = waterGuideBoardNeedles(frame, 180);
    assert.equal(board.targetVisible, false);
    assert.equal(board.targetBearingDeg, null);
    assert.equal(board.movementVisible, false);
    assert.equal(board.movementBearingDeg, null);
  }
});
