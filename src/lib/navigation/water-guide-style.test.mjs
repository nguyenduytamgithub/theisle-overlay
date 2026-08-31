import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const css = readFileSync(
  new URL("../../water-guide/style.css", import.meta.url),
  "utf8",
);
const main = readFileSync(
  new URL("../../water-guide/main.ts", import.meta.url),
  "utf8",
);

test("the aligned green ray overrides off-route and lost colors", () => {
  const alignedGreenRule = css.lastIndexOf(
    '.water-guide[data-aligned="true"] .ray-line',
  );
  const offRouteLostRule = css.lastIndexOf(
    '.water-guide[data-state="lost"] .ray-line',
  );

  assert.ok(alignedGreenRule >= 0, "aligned ray rule is missing");
  assert.ok(offRouteLostRule >= 0, "off-route/lost ray rule is missing");
  assert.ok(
    alignedGreenRule > offRouteLostRule,
    "aligned green rule must come later so it wins the CSS cascade",
  );
});

test("the Water Guide ignores head/camera direction and renders a static screen ray", () => {
  assert.doesNotMatch(
    main,
    /NavigationEstimator|guidanceCourseDeg|serverFacingDeg|motionCourseDeg/,
  );
  assert.match(main, /movementCourseBetween/);
  assert.match(main, /freshnessForAge/);
  assert.doesNotMatch(css, /rotate\s*\(/);
  assert.doesNotMatch(css, /animation\s*:/);
  assert.doesNotMatch(css, /@keyframes/);
  assert.doesNotMatch(main, /--ray-angle/);
});
