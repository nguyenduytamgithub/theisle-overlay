import { emit } from "@tauri-apps/api/event";
import { error } from "@tauri-apps/plugin-log";

import { installGlobalErrorLog } from "../lib/errlog";

installGlobalErrorLog("night-vision-filter");

async function init() {
  await emit("night-vision-filter://ready", {});
  await emit("night-vision-filter://heartbeat", {});
  window.setInterval(() => {
    void emit("night-vision-filter://heartbeat", {});
  }, 2_000);
}

void init().catch((reason) => {
  void error(`[night-vision-filter] init failed: ${reason}`).catch(() => {});
});
