// Global error capture -> the app's log file (%LOCALAPPDATA%\TheIsleOverlay\
// logs). Webviews have no devtools in the field, so without this every
// uncaught error and unhandled rejection simply vanishes.

import { error } from "@tauri-apps/plugin-log";

export function installGlobalErrorLog(label: string): void {
  window.addEventListener("error", (e) => {
    void error(`[${label}] ${e.message} @ ${e.filename}:${e.lineno}`).catch(() => {});
  });
  window.addEventListener("unhandledrejection", (e) => {
    void error(`[${label}] unhandled rejection: ${String(e.reason)}`).catch(() => {});
  });
}
