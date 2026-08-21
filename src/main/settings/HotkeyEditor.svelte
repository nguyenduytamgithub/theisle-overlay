<script lang="ts">
  // Hotkey rebinding rows. Click a field, press the combination; validation:
  // must parse (>= 1 modifier), not duplicate another action, and — probed
  // through RegisterHotKey in Rust — not held by another app.
  import {
    applyHotkeys,
    checkHotkeyAvailable,
    patchSettings,
    type Settings,
  } from "$lib/api";
  import { t } from "$lib/i18n";

  let { settings, onchanged }: { settings: Settings; onchanged: (s: Settings) => void } =
    $props();

  const ACTIONS = [
    "toggle_minimap",
    "toggle_fullmap",
    "toggle_click_through",
    "mark_here",
    "opacity_up",
    "opacity_down",
    "zoom_in",
    "zoom_out",
    "reload_ui",
  ] as const;

  let capturing = $state<string | null>(null);
  let errors = $state<Record<string, string>>({});

  const KEY_NAMES: Record<string, string> = {
    ArrowLeft: "Left",
    ArrowUp: "Up",
    ArrowRight: "Right",
    ArrowDown: "Down",
    " ": "Space",
    Enter: "Enter",
    Tab: "Tab",
    Insert: "Insert",
    Delete: "Delete",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
    "+": "Plus",
    "-": "Minus",
    "=": "Plus",
  };

  function specFromEvent(e: KeyboardEvent): string | null {
    const mods: string[] = [];
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.altKey) mods.push("Alt");
    if (e.shiftKey) mods.push("Shift");
    if (e.metaKey) mods.push("Win");
    const key = e.key;
    if (["Control", "Alt", "Shift", "Meta"].includes(key)) return null; // modifier only
    let name: string | null = null;
    if (key in KEY_NAMES) name = KEY_NAMES[key];
    else if (/^[a-zA-Z0-9]$/.test(key)) name = key.toUpperCase();
    else if (/^F\d{1,2}$/.test(key)) name = key;
    if (!name || mods.length === 0) return null;
    return [...mods, name].join("+");
  }

  async function onCaptureKey(action: string, e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      capturing = null;
      return;
    }
    const spec = specFromEvent(e);
    if (!spec) {
      if (!["Control", "Alt", "Shift", "Meta"].includes(e.key)) {
        errors = { ...errors, [action]: $t("settings.hotkey_invalid") };
      }
      return;
    }
    // Duplicate inside the app?
    const hotkeys = settings.hotkeys as Record<string, string>;
    const dup = Object.entries(hotkeys).find(([a, s]) => a !== action && s === spec);
    if (dup) {
      errors = { ...errors, [action]: $t("settings.hotkey_duplicate") };
      return;
    }
    // Held by another app? (Skip the probe when unchanged — our own live
    // registration would make the probe fail against ourselves.)
    if (hotkeys[action] !== spec && !(await checkHotkeyAvailable(spec))) {
      errors = { ...errors, [action]: $t("settings.hotkey_in_use") };
      return;
    }
    const { [action]: _removed, ...rest } = errors;
    errors = rest;
    capturing = null;
    const next = await patchSettings({ hotkeys: { [action]: spec } });
    await applyHotkeys();
    onchanged(next);
  }
</script>

<div class="space-y-1">
  <p class="text-xs" style="color: var(--color-muted)">{$t("settings.hotkeys_hint")}</p>
  {#each ACTIONS as action (action)}
    <div class="flex items-center gap-2 text-sm">
      <span class="w-56">{$t(`hotkey.${action}` as never)}</span>
      <button
        class="w-44 cursor-pointer rounded border px-2 py-1 text-left font-mono text-xs"
        style="border-color: {capturing === action
          ? 'var(--color-accent)'
          : 'var(--color-border)'}; background: var(--color-bg)"
        onclick={() => {
          capturing = action;
        }}
        onkeydown={(e) => {
          if (capturing === action) void onCaptureKey(action, e);
        }}
        onblur={() => {
          if (capturing === action) capturing = null;
        }}
      >
        {capturing === action
          ? $t("settings.press_keys")
          : ((settings.hotkeys as Record<string, string>)[action] ?? "—")}
      </button>
      {#if errors[action]}
        <span class="text-xs" style="color: #ff8a80">{errors[action]}</span>
      {/if}
    </div>
  {/each}
</div>
