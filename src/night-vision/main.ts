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
      ? "Màn hình hoặc chế độ HDR hiện tại không nhận chỉnh gamma."
      : "The current display or HDR mode rejected gamma adjustment.";
  }
  if (value.requested && !value.applied) {
    return language === "vi"
      ? "Đã yêu cầu; đang chờ The Isle ở màn hình trước."
      : "Requested; waiting for The Isle to be foreground.";
  }
  return language === "vi"
    ? `Cường độ ${value.strength}%. Bấm hoặc nhấn Ctrl+Alt+N.`
    : `Strength ${value.strength}%. Click or press Ctrl+Alt+N.`;
}

function render() {
  button.disabled = busy;
  button.className = state.applied
    ? "on"
    : !state.supported
      ? "unavailable"
      : state.requested
        ? "waiting"
        : "off";
  button.setAttribute("aria-pressed", String(state.applied));
  button.title = stateTitle(state);
  if (!state.supported) {
    label.textContent = language === "vi" ? "KHÔNG HỖ TRỢ" : "UNAVAILABLE";
  } else if (state.applied) {
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
}

void init().catch((reason) => {
  void error(`[night-vision] init failed: ${reason}`).catch(() => {});
  void emit("night-vision://ready", {});
});
