// Locale store + t(). Same crash-proofing rule as the original strings_vi.py:
// a missing key returns the key itself — a typo must never crash the overlay
// mid-game.

import { derived, writable, get } from "svelte/store";
import { vi, type MsgKey } from "./vi";
import { en } from "./en";

export type Locale = "vi" | "en";
export const locale = writable<Locale>("vi");

const DICTS: Record<Locale, Record<MsgKey, string>> = { vi, en };

function translate(l: Locale, key: MsgKey, params?: Record<string, string | number>): string {
  let text: string = DICTS[l][key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      text = text.replaceAll(`{${k}}`, String(v));
    }
  }
  return text;
}

/** Reactive translator: `{$t("layer.water")}` re-renders on locale change. */
export const t = derived(
  locale,
  (l) =>
    (key: MsgKey, params?: Record<string, string | number>): string =>
      translate(l, key, params),
);

/** Non-reactive translation for imperative code. */
export function tNow(key: MsgKey, params?: Record<string, string | number>): string {
  return translate(get(locale), key, params);
}

/** Compass key from Rust ("dir.N" ...) -> localised label. */
export function compassLabel(l: Locale, key: string | null): string {
  if (!key) return "";
  return key in DICTS[l] ? DICTS[l][key as MsgKey] : key;
}

/** Human distance: metres below 1 km, else km. Locale-aware separators. */
export function formatDistance(l: Locale, metres: number): string {
  const numberLocale = l === "vi" ? "vi-VN" : "en-US";
  if (metres < 1000) {
    return `${Math.round(metres).toLocaleString(numberLocale)} m`;
  }
  return `${(metres / 1000).toLocaleString(numberLocale, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })} km`;
}
