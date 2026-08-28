import assert from "node:assert/strict";
import test from "node:test";

import {
  advanceAngleDeg,
  effectiveProjectionAgeS,
  freshnessForAge,
  NavigationEstimator,
  shortestDeltaDeg,
} from "./estimator.ts";

const sample = (overrides = {}) => ({
  xCm: 0,
  yCm: 0,
  px: 0,
  py: 0,
  velocityXCmS: null,
  velocityYCmS: null,
  velocityPxXS: null,
  velocityPxYS: null,
  serverFacingDeg: null,
  motionCourseDeg: null,
  confirmedAtMs: 0,
  relocated: false,
  ...overrides,
});

const estimatorWithEastTarget = () => {
  const nav = new NavigationEstimator();
  nav.setTarget({ id: "east", name: "East", xCm: 0, yCm: 10_000 });
  return nav;
};

const estimatorWithVelocity = (velocityXCmS) => {
  const nav = new NavigationEstimator();
  nav.accept(sample({ velocityXCmS, velocityYCmS: 0, velocityPxXS: velocityXCmS / 100, velocityPxYS: 0 }));
  return nav;
};

const estimatorWithTargetAt = (xCm, yCm) => {
  const nav = new NavigationEstimator();
  nav.setTarget({ id: "target", name: "Target", xCm, yCm });
  return nav;
};

const acceptAtDistances = (nav, distancesM) => {
  distancesM.forEach((distanceM, index) => {
    nav.accept(sample({
      xCm: 100_000 - distanceM * 100,
      confirmedAtMs: index * 15_000,
    }));
  });
};

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

test("noisy server facing cannot rotate the absolute target arrow", () => {
  const nav = estimatorWithEastTarget();
  nav.accept(sample({ serverFacingDeg: 10, confirmedAtMs: 0 }));
  const first = nav.snapshot(0);
  nav.accept(sample({ serverFacingDeg: 280, confirmedAtMs: 5_000 }));
  const second = nav.snapshot(5_000);
  assert.equal(first.targetBearingDeg, 90);
  assert.equal(second.targetBearingDeg, 90);
});

test("sixteen-second gap decays and holds instead of freezing then jumping", () => {
  const nav = estimatorWithVelocity(100);
  assert.equal(Math.round(nav.snapshot(4_000).xCm), 400);
  assert.equal(Math.round(nav.snapshot(12_000).xCm), 679);
  assert.equal(Math.round(nav.snapshot(16_000).xCm), 679);
});

test("arrival freezes guidance inside twenty-five metres", () => {
  const nav = estimatorWithTargetAt(2_400, 0);
  nav.accept(sample({ xCm: 0, yCm: 0 }));
  const view = nav.snapshot(0);
  assert.equal(view.arrived, true);
  assert.equal(view.maneuver, "arrived");
});

test("three confirmations with under ten metres progress warn once", () => {
  const nav = estimatorWithTargetAt(100_000, 0);
  acceptAtDistances(nav, [1000, 998, 995]);
  assert.equal(nav.snapshot(30_000).noProgress, true);
});

test("course source must remain valid for one second before switching", () => {
  const nav = estimatorWithEastTarget();
  nav.accept(sample({ serverFacingDeg: 0, confirmedAtMs: 0 }));
  assert.equal(nav.snapshot(999).guidanceCourseDeg, null);
  assert.equal(nav.snapshot(1_000).guidanceCourseDeg, 0);

  nav.accept(sample({
    velocityXCmS: 100,
    velocityYCmS: 0,
    velocityPxXS: 1,
    velocityPxYS: 0,
    motionCourseDeg: 90,
    serverFacingDeg: 0,
    confirmedAtMs: 2_000,
  }));
  assert.equal(nav.snapshot(2_999).guidanceCourseDeg, 0);
  const switched = nav.snapshot(3_000).guidanceCourseDeg;
  assert.ok(switched > 0 && switched < 1);
  assert.ok(Math.abs(nav.snapshot(3_500).guidanceCourseDeg - 60.12) < 0.001);
  assert.equal(nav.snapshot(3_750).guidanceCourseDeg, 90);
});

test("maneuver changes only after six hundred milliseconds of stability", () => {
  const nav = estimatorWithEastTarget();
  nav.accept(sample({ serverFacingDeg: 90, confirmedAtMs: 0 }));
  assert.equal(nav.snapshot(1_000).maneuver, "straight");
  nav.accept(sample({ serverFacingDeg: 0, confirmedAtMs: 2_000 }));
  assert.equal(nav.snapshot(2_000).maneuver, "straight");
  assert.equal(nav.snapshot(2_599).maneuver, "straight");
  assert.equal(nav.snapshot(2_600).maneuver, "right");
});

test("ordinary correction eases over 650 ms", () => {
  const nav = new NavigationEstimator();
  nav.accept(sample());
  nav.snapshot(0);
  nav.accept(sample({ xCm: 2_000, px: 20, confirmedAtMs: 5_000 }));
  assert.equal(nav.snapshot(5_325).xCm, 1_000);
  assert.equal(nav.snapshot(5_650).xCm, 2_000);
});

test("medium correction eases over 300 ms", () => {
  const nav = new NavigationEstimator();
  nav.accept(sample());
  nav.snapshot(0);
  nav.accept(sample({ xCm: 5_000, px: 50, confirmedAtMs: 5_000 }));
  assert.equal(nav.snapshot(5_150).xCm, 2_500);
  assert.equal(nav.snapshot(5_300).xCm, 5_000);
});

test("large or explicit relocation snaps and resets projection", () => {
  const large = new NavigationEstimator();
  large.accept(sample());
  large.snapshot(0);
  large.accept(sample({ xCm: 11_000, px: 110, confirmedAtMs: 5_000 }));
  assert.equal(large.snapshot(5_000).xCm, 11_000);

  const relocated = new NavigationEstimator();
  relocated.accept(sample());
  relocated.snapshot(0);
  relocated.accept(sample({ xCm: 2_000, px: 20, confirmedAtMs: 5_000, relocated: true }));
  assert.equal(relocated.snapshot(5_000).xCm, 2_000);
});
