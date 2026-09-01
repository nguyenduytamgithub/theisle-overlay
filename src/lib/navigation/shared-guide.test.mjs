import assert from "node:assert/strict";
import test from "node:test";

import {
  hasSelectedWaypoint,
  sharedGuideRequested,
} from "./shared-guide.ts";

test("the shared XY board is requested by water or a selected waypoint", () => {
  assert.equal(sharedGuideRequested(false, false), false);
  assert.equal(sharedGuideRequested(true, false), true);
  assert.equal(sharedGuideRequested(false, true), true);
  assert.equal(sharedGuideRequested(true, true), true);
});

test("only a non-empty waypoint id requests waypoint guidance", () => {
  assert.equal(hasSelectedWaypoint({}), false);
  assert.equal(hasSelectedWaypoint({ navigation: {} }), false);
  assert.equal(hasSelectedWaypoint({ navigation: { target_waypoint_id: "   " } }), false);
  assert.equal(hasSelectedWaypoint({ navigation: { target_waypoint_id: "home" } }), true);
});
