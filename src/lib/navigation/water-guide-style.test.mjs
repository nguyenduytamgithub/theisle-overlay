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
const html = readFileSync(
  new URL("../../../water-guide.html", import.meta.url),
  "utf8",
);

test("the game overlay contains a north-up XY board instead of a ray", () => {
  assert.match(html, /id="board"/);
  assert.match(html, /id="target-needle"/);
  assert.match(html, /id="movement-needle"/);
  assert.match(html, />BẮC</);
  assert.match(html, />ĐÔNG</);
  assert.match(html, />NAM</);
  assert.match(html, />TÂY</);
  assert.doesNotMatch(html, /id="ray"/);
});

test("the Water Guide renders absolute XY needles without camera inputs", () => {
  assert.doesNotMatch(
    main,
    /NavigationEstimator|guidanceCourseDeg|serverFacingDeg|motionCourseDeg/,
  );
  assert.match(main, /movementCourseBetween/);
  assert.match(main, /waterGuideBoardNeedles/);
  assert.match(main, /freshnessForAge/);
  assert.match(main, /--target-bearing/);
  assert.match(main, /--movement-bearing/);
  assert.doesNotMatch(main, /--ray-angle/);
  assert.doesNotMatch(css, /animation\s*:/);
  assert.doesNotMatch(css, /@keyframes/);
  assert.doesNotMatch(css, /transition\s*:[^;]*transform/);
});

test("aligned XY movement is green while the cyan target remains distinct", () => {
  assert.match(css, /\.target-needle[\s\S]*#(?:19c6ff|00bfff|52d7ff)/i);
  assert.match(
    css,
    /\.water-guide\[data-aligned="true"\][\s\S]*\.movement-needle/,
  );
});

test("position-quality loss freezes the last XY needles instead of clearing course", () => {
  const listener = main.match(
    /await listen\("position:\/\/quality",[\s\S]*?\n  \}\);/,
  );

  assert.ok(listener, "position quality listener is missing");
  assert.doesNotMatch(listener[0], /resetMovementCourse/);
  assert.match(listener[0], /positionQualityValid = false/);
});
