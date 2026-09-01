import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = (relativePath) => readFileSync(
  new URL(relativePath, import.meta.url),
  "utf8",
);

test("the shared XY board suppresses the heading HUD for water or a waypoint", () => {
  const hud = source("../../hud/main.ts");

  assert.match(hud, /listen<WaterGuideSnapshot>\("water-guide:\/\/changed"/);
  assert.match(hud, /hasSelectedWaypoint\(settings\)/);
  assert.match(hud, /sharedGuideRequested\(waterGuideRequested, waypointGuideRequested\)/);
  assert.match(hud, /hud\.hidden\s*=\s*hidden/);
  assert.match(hud, /invoke<WaterGuideSnapshot>\("get_water_guide_state"\)/);
  assert.match(hud, /waterGuideStateRevision \+= 1/);
  assert.match(hud, /waterGuideStateRevision === revisionBeforeSnapshot/);
});

test("the shared XY board suppresses the rotating minimap for water or a waypoint", () => {
  const minimap = source("../../minimap/main.ts");

  assert.match(minimap, /listen<WaterGuideSnapshot>\("water-guide:\/\/changed"/);
  assert.match(minimap, /hasSelectedWaypoint\(s\)/);
  assert.match(minimap, /sharedGuideRequested\(waterGuideRequested, waypointGuideRequested\)/);
  assert.match(minimap, /canvas\.style\.display\s*=\s*hidden \? "none" : "block"/);
  assert.match(minimap, /invoke<WaterGuideSnapshot>\("get_water_guide_state"\)/);
  assert.match(minimap, /waterGuideStateRevision \+= 1/);
  assert.match(minimap, /waterGuideStateRevision === revisionBeforeSnapshot/);
});

test("HUD and minimap register settings listeners before guarded snapshots", () => {
  const hud = source("../../hud/main.ts");
  const minimap = source("../../minimap/main.ts");

  for (const [label, appSource] of [["HUD", hud], ["minimap", minimap]]) {
    const listener = appSource.indexOf('"settings://changed"');
    const snapshot = appSource.indexOf('"get_settings"');
    assert.ok(listener >= 0, `${label} settings listener is missing`);
    assert.ok(snapshot >= 0, `${label} settings snapshot is missing`);
    assert.ok(listener < snapshot, `${label} listens too late for settings changes`);
    assert.match(appSource, /let settingsRevision = 0/);
    assert.match(appSource, /settingsRevision \+= 1/);
    assert.match(
      appSource,
      /settingsRevisionBeforeSnapshot === 0\s*&&\s*settingsRevision === settingsRevisionBeforeSnapshot/,
    );
  }
});
