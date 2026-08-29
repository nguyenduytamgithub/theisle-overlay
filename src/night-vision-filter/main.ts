import { emit, listen } from "@tauri-apps/api/event";
import { error } from "@tauri-apps/plugin-log";

import { installGlobalErrorLog } from "../lib/errlog";

installGlobalErrorLog("night-vision-filter");

interface FilterPaintRequest {
  requestId: number;
  strength: number;
  alpha: number;
  color: string;
}

interface FilterPainted {
  requestId: number;
  strength: number;
}

const veil = document.getElementById("veil")!;

function nextFrame(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

async function paint(request: FilterPaintRequest) {
  if (!Number.isSafeInteger(request.requestId) || request.requestId <= 0) return;

  const strength = Math.round(Math.min(100, Math.max(0, request.strength)));
  const alpha = Math.min(0.3, Math.max(0, request.alpha));
  veil.style.backgroundColor = request.color;
  veil.style.opacity = String(alpha);

  await nextFrame();
  await nextFrame();
  await emit<FilterPainted>("night-vision-filter://painted", {
    requestId: request.requestId,
    strength,
  });
}

async function init() {
  await listen<FilterPaintRequest>("night-vision-filter://paint", (event) => {
    void paint(event.payload).catch((reason) => {
      void error(`[night-vision-filter] paint failed: ${reason}`).catch(() => {});
    });
  });

  await emit("night-vision-filter://ready", {});
  await emit("night-vision-filter://heartbeat", {});
  window.setInterval(() => {
    void emit("night-vision-filter://heartbeat", {});
  }, 2_000);
}

void init().catch((reason) => {
  void error(`[night-vision-filter] init failed: ${reason}`).catch(() => {});
});
