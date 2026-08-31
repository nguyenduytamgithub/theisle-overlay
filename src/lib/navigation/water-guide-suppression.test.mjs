import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = (relativePath) => readFileSync(
  new URL(relativePath, import.meta.url),
  "utf8",
);

test("Water Guide suppresses the heading HUD while its XY ray is requested", () => {
  const hud = source("../../hud/main.ts");

  assert.match(hud, /listen<WaterGuideSnapshot>\("water-guide:\/\/changed"/);
  assert.match(hud, /hud\.hidden\s*=\s*requested/);
  assert.match(hud, /invoke<WaterGuideSnapshot>\("get_water_guide_state"\)/);
});

test("Water Guide suppresses the rotating minimap while its XY ray is requested", () => {
  const minimap = source("../../minimap/main.ts");

  assert.match(minimap, /listen<WaterGuideSnapshot>\("water-guide:\/\/changed"/);
  assert.match(minimap, /canvas\.hidden\s*=\s*requested/);
  assert.match(minimap, /invoke<WaterGuideSnapshot>\("get_water_guide_state"\)/);
});
