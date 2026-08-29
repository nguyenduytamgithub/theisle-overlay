import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { error } from "@tauri-apps/plugin-log";

import { installGlobalErrorLog } from "../lib/errlog";

installGlobalErrorLog("night-vision");

interface NightVisionState {
  requested: boolean;
  applied: boolean;
  supported: boolean;
  strength: number;
  errorKey: string | null;
  visualBoostReady: boolean;
  visualBoostApplied: boolean;
  gammaApplied: boolean;
  buildFingerprint: string;
}

type Language = "vi" | "en";

const button = document.getElementById("night-vision") as HTMLButtonElement;
const label = document.getElementById("label")!;

let language: Language = "vi";
let busy = false;
let state: NightVisionState = {
  requested: false,
  applied: false,
  supported: true,
  strength: 70,
  errorKey: null,
  visualBoostReady: false,
  visualBoostApplied: false,
  gammaApplied: false,
  buildFingerprint: "loading",
};

function applySettings(settings: Record<string, unknown>) {
  language = settings.language === "en" ? "en" : "vi";
}

function stateTitle(value: NightVisionState): string {
  if (value.errorKey === "night_vision.recovery_error") {
    return language === "vi"
      ? "Không thể phục hồi màu màn hình từ lần chạy trước. Hãy khởi động lại ứng dụng."
      : "Could not restore the display from the previous run. Restart the app.";
  }
  if (!value.supported) {
    return language === "vi"
      ? "Lớp tăng sáng chưa hoạt động; nút này chưa được phép báo BẬT."
      : "The visual boost is not active; this button cannot report ON yet.";
  }
  if (value.requested && !value.visualBoostApplied) {
    return language === "vi"
      ? "Đã yêu cầu; đang chờ The Isle ở màn hình trước."
      : "Requested; waiting for The Isle to be foreground.";
  }
  if (value.visualBoostApplied && !value.gammaApplied) {
    return language === "vi"
      ? `Lớp tăng sáng đã bật ở ${value.strength}%. Gamma phụ trợ không được driver nhận.`
      : `Visual boost is on at ${value.strength}%. Supplemental gamma was not accepted.`;
  }
  return language === "vi"
    ? `Cường độ ${value.strength}%. Bấm hoặc nhấn Ctrl+Alt+N.`
    : `Strength ${value.strength}%. Click or press Ctrl+Alt+N.`;
}

function render() {
  button.disabled = busy;
  button.className = state.visualBoostApplied
    ? "on"
    : !state.supported
      ? "unavailable"
      : state.requested
        ? "waiting"
        : "off";
  button.setAttribute("aria-pressed", String(state.visualBoostApplied));
  button.title = stateTitle(state);
  if (!state.supported) {
    label.textContent = language === "vi" ? "KHÔNG HỖ TRỢ" : "UNAVAILABLE";
  } else if (state.visualBoostApplied) {
    label.textContent = language === "vi" ? "NHÌN ĐÊM: BẬT" : "NIGHT VISION: ON";
  } else {
    label.textContent = language === "vi" ? "NHÌN ĐÊM: TẮT" : "NIGHT VISION: OFF";
  }
}

button.addEventListener("click", async () => {
  if (busy) return;
  busy = true;
  render();
  try {
    state = await invoke<NightVisionState>("toggle_night_vision");
  } catch (reason) {
    await error(`[night-vision] toggle failed: ${reason}`).catch(() => {});
  } finally {
    busy = false;
    render();
  }
});

async function init() {
  await listen<NightVisionState>("night-vision://changed", (event) => {
    state = event.payload;
    render();
  });
  await listen<Record<string, unknown>>("settings://changed", (event) => {
    applySettings(event.payload);
    render();
  });

  const settings = await invoke<Record<string, unknown>>("get_settings");
  applySettings(settings);
  state = await invoke<NightVisionState>("get_night_vision_state");
  render();
  await emit("night-vision://ready", {});
  await emit("night-vision://heartbeat", {});
  window.setInterval(() => {
    void emit("night-vision://heartbeat", {});
  }, 2_000);
}

void init().catch((reason) => {
  void error(`[night-vision] init failed: ${reason}`).catch(() => {});
});
