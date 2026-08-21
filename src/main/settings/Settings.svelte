<script lang="ts">
  // Settings screen — the UI the old app never had (hotkeys were edited by
  // hand in settings.json). Every control writes through patch_settings, so
  // the minimap window and the Rust supervisor react live.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    getSettings,
    listenerBag,
    onFetchFinished,
    patchSettings,
    setBasemapSource,
    startFetchData,
    type BasemapSource,
    type Settings,
  } from "$lib/api";
  import { t } from "$lib/i18n";
  import HotkeyEditor from "./HotkeyEditor.svelte";

  let settings = $state<Settings | null>(null);
  let refetching = $state(false);

  onMount(() => {
    const bag = listenerBag();
    void getSettings().then((s) => (settings = s));
    void bag.add(onFetchFinished(() => (refetching = false)));
    return () => bag.dispose();
  });

  function redownload() {
    refetching = true;
    void startFetchData(true);
  }

  async function patch(p: object) {
    settings = await patchSettings(p);
  }

  // Basemap switch: the command downloads the imagery on first selection, so
  // the active pill only moves after the command succeeds — a failed
  // (offline) download leaves settings and UI exactly as they were.
  let basemapBusy = $state<BasemapSource | null>(null);
  let basemapError = $state(false);

  async function chooseBasemap(source: BasemapSource) {
    if (basemapBusy || settings?.map.basemap === source) return;
    basemapBusy = source;
    basemapError = false;
    try {
      await setBasemapSource(source);
      settings = await getSettings();
    } catch {
      basemapError = true;
    } finally {
      basemapBusy = null;
    }
  }

  const BASEMAPS = ["vulnona", "islemaps_light", "islemaps_dark"] as const;

  const CORNERS = ["top-left", "top-right", "bottom-left", "bottom-right"] as const;

  const openTrails = () => invoke("open_trails_folder");
</script>

{#if settings}
  <div class="mx-auto max-w-2xl space-y-6 overflow-y-auto p-6">
    <!-- Language -->
    <section>
      <h2 class="mb-2 font-semibold" style="color: var(--color-accent)">
        {$t("settings.language")}
      </h2>
      <div class="flex gap-2">
        {#each [["vi", "Tiếng Việt"], ["en", "English"]] as [code, label] (code)}
          <button
            class="cursor-pointer rounded border px-3 py-1 text-sm"
            style={settings.language === code
              ? "background: var(--color-accent); color: var(--color-bg); border-color: var(--color-accent); font-weight: 600"
              : "border-color: var(--color-border)"}
            onclick={() => void patch({ language: code })}
          >
            {label}
          </button>
        {/each}
      </div>
    </section>

    <!-- Minimap -->
    <section>
      <h2 class="mb-2 font-semibold" style="color: var(--color-accent)">
        {$t("settings.minimap")}
      </h2>
      <div class="space-y-3">
        <label class="flex cursor-pointer items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.minimap.visible}
            onchange={(e) => void patch({ minimap: { visible: e.currentTarget.checked } })}
          />
          {$t("settings.visible")}
        </label>
        <label class="flex cursor-pointer items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.minimap.require_game}
            onchange={(e) =>
              void patch({ minimap: { require_game: e.currentTarget.checked } })}
          />
          {$t("settings.require_game")}
        </label>
        <label class="flex cursor-pointer items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.minimap.click_through}
            onchange={(e) =>
              void patch({ minimap: { click_through: e.currentTarget.checked } })}
          />
          {$t("settings.click_through")}
        </label>
        <label class="flex cursor-pointer items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.minimap.show_trail ?? true}
            onchange={(e) =>
              void patch({ minimap: { show_trail: e.currentTarget.checked } })}
          />
          {$t("settings.show_trail")}
        </label>
        <label class="flex cursor-pointer items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={settings.minimap.show_waypoints ?? true}
            onchange={(e) =>
              void patch({ minimap: { show_waypoints: e.currentTarget.checked } })}
          />
          {$t("settings.show_waypoints")}
        </label>

        <div class="text-sm">
          <div class="mb-1">{$t("settings.corner")}</div>
          <div class="grid w-40 grid-cols-2 gap-1">
            {#each CORNERS as corner (corner)}
              <button
                class="cursor-pointer rounded border px-2 py-1 text-xs"
                style={settings.minimap.corner === corner
                  ? "background: var(--color-accent); color: var(--color-bg); border-color: var(--color-accent); font-weight: 600"
                  : "border-color: var(--color-border)"}
                onclick={() => void patch({ minimap: { corner } })}
              >
                {$t(`corner.${corner}` as never)}
              </button>
            {/each}
          </div>
        </div>

        {#each [["size_px", "settings.size", 180, 400, 10, "px"], ["margin_px", "settings.margin", 0, 64, 2, "px"], ["opacity", "settings.opacity", 0.25, 1, 0.05, ""], ["radius_m", "settings.radius", 150, 3000, 50, "m"]] as [key, labelKey, min, max, step, unit] (key)}
          <label class="block text-sm">
            <div class="mb-0.5 flex justify-between">
              <span>{$t(labelKey as never)}</span>
              <span class="font-mono" style="color: var(--color-muted)">
                {key === "opacity"
                  ? `${Math.round((settings.minimap.opacity as number) * 100)}%`
                  : `${(settings.minimap as never as Record<string, number>)[key as string]} ${unit}`}
              </span>
            </div>
            <input
              type="range"
              class="w-full accent-[#e8a33d]"
              min={min as number}
              max={max as number}
              step={step as number}
              value={(settings.minimap as never as Record<string, number>)[key as string]}
              oninput={(e) =>
                void patch({ minimap: { [key as string]: Number(e.currentTarget.value) } })}
            />
          </label>
        {/each}
      </div>
    </section>

    <!-- Hotkeys -->
    <section>
      <h2 class="mb-2 font-semibold" style="color: var(--color-accent)">
        {$t("settings.hotkeys")}
      </h2>
      <HotkeyEditor {settings} onchanged={(s) => (settings = s)} />
    </section>

    <!-- Number format -->
    <section>
      <h2 class="mb-2 font-semibold" style="color: var(--color-accent)">
        {$t("settings.number_format")}
      </h2>
      <div class="flex gap-2">
        {#each ["auto", "us", "eu"] as fmt (fmt)}
          <button
            class="cursor-pointer rounded border px-3 py-1 text-sm"
            style={settings.number_format === fmt
              ? "background: var(--color-accent); color: var(--color-bg); border-color: var(--color-accent); font-weight: 600"
              : "border-color: var(--color-border)"}
            onclick={() => void patch({ number_format: fmt })}
          >
            {$t(`format.${fmt}` as never)}
          </button>
        {/each}
      </div>
    </section>

    <!-- Basemap style -->
    <section>
      <h2 class="mb-2 font-semibold" style="color: var(--color-accent)">
        {$t("settings.basemap")}
      </h2>
      <div class="flex flex-wrap gap-2">
        {#each BASEMAPS as source (source)}
          <button
            class="cursor-pointer rounded border px-3 py-1 text-sm disabled:opacity-50"
            style={settings.map.basemap === source
              ? "background: var(--color-accent); color: var(--color-bg); border-color: var(--color-accent); font-weight: 600"
              : "border-color: var(--color-border)"}
            disabled={basemapBusy !== null}
            onclick={() => void chooseBasemap(source)}
          >
            {basemapBusy === source
              ? $t("basemap.downloading")
              : $t(`basemap.${source}` as never)}
          </button>
        {/each}
      </div>
      {#if basemapError}
        <p class="mt-2 text-xs" style="color: #ff8a80">{$t("basemap.failed")}</p>
      {/if}
      <p class="mt-2 text-xs leading-relaxed" style="color: var(--color-muted)">
        {$t("basemap.hint")}
      </p>
    </section>

    <!-- Data -->
    <section>
      <h2 class="mb-2 font-semibold" style="color: var(--color-accent)">
        {$t("settings.data")}
      </h2>
      <div class="flex gap-2">
        <button
          class="cursor-pointer rounded border px-3 py-1 text-sm"
          style="border-color: var(--color-border)"
          onclick={() => void openTrails()}
        >
          {$t("settings.open_trails")}
        </button>
        <button
          class="cursor-pointer rounded border px-3 py-1 text-sm disabled:opacity-50"
          style="border-color: var(--color-border)"
          disabled={refetching}
          onclick={redownload}
        >
          {refetching ? $t("firstrun.downloading") : $t("settings.redownload")}
        </button>
      </div>
      <div class="mt-3 text-xs leading-relaxed" style="color: var(--color-muted)">
        <div class="mb-1 font-semibold">{$t("credits.title")}</div>
        {$t("credits.body")}
      </div>
    </section>
  </div>
{/if}
