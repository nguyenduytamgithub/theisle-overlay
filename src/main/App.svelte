<script lang="ts">
  // Main window shell: tab navigation (map | settings | guide), the
  // exclusive-fullscreen warning banner, and locale bootstrapping.
  import { onMount } from "svelte";
  import {
    getDataStatus,
    getFullscreenMode,
    getSettings,
    onFetchFinished,
    onHotkeyFailed,
    onSettingsChanged,
    simulatePosition,
    type DataStatus,
    type FailedHotkey,
  } from "$lib/api";
  import { locale, t, type Locale } from "$lib/i18n";
  import FullMap from "./fullmap/FullMap.svelte";
  import DinoTab from "./dino/DinoTab.svelte";
  import Settings from "./settings/Settings.svelte";
  import Guide from "./guide/Guide.svelte";
  import FirstRun from "./firstrun/FirstRun.svelte";

  type Tab = "map" | "dino" | "settings" | "guide";
  const initialTab = ["map", "dino", "settings", "guide"].includes(location.hash.slice(1))
    ? (location.hash.slice(1) as Tab)
    : "map";
  let tab = $state<Tab>(initialTab);
  let dataStatus = $state<DataStatus | null>(null);
  let exclusiveFullscreen = $state(false);
  let failedHotkeys = $state<FailedHotkey[]>([]);
  let ready = $state(false);

  // Update prompt: silent check on launch, non-blocking banner, only ever in
  // this window — never over the game.
  let updateVersion = $state<string | null>(null);
  let updating = $state(false);
  let pendingUpdate: import("@tauri-apps/plugin-updater").Update | null = null;

  async function checkForUpdate() {
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (update) {
        pendingUpdate = update;
        updateVersion = update.version;
      }
    } catch {
      // Offline or endpoint not set up yet — stay silent.
    }
  }

  async function installUpdate() {
    if (!pendingUpdate) return;
    updating = true;
    try {
      await pendingUpdate.downloadAndInstall();
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch {
      updating = false;
    }
  }

  // POIs are optional (fail-soft: the map works without dots); the basemap
  // images are the hard requirement.
  const dataOk = $derived(
    dataStatus !== null && dataStatus.basemapMinimap && dataStatus.basemapFullmap,
  );

  onMount(() => {
    const unlisteners: Array<() => void> = [];
    (async () => {
      const settings = await getSettings();
      locale.set((settings.language as Locale) ?? "vi");
      dataStatus = await getDataStatus();
      exclusiveFullscreen = (await getFullscreenMode()) === 0;
      unlisteners.push(
        await onSettingsChanged((s) => locale.set((s.language as Locale) ?? "vi")),
      );
      unlisteners.push(await onHotkeyFailed((failed) => (failedHotkeys = failed)));
      // The download can finish while the user is on another tab (FirstRun
      // unmounted) — the App itself must notice and unlock the map tab.
      unlisteners.push(
        await onFetchFinished(() => void getDataStatus().then((d) => (dataStatus = d))),
      );
      ready = true;
      void checkForUpdate();
    })();
    return () => unlisteners.forEach((u) => u());
  });

  // Dev-only: walk south-east to exercise the pipeline without the game.
  let simX = -231654;
  function simulateStep() {
    simX += 30_000;
    void simulatePosition(simX, 52099.673, 0);
  }
</script>

<div class="flex h-screen flex-col">
  <header
    class="flex shrink-0 items-center gap-1 border-b px-3 py-1.5"
    style="border-color: var(--color-border); background: var(--color-panel)"
  >
    <span class="mr-3 font-semibold" style="color: var(--color-accent)">
      {$t("app.title")}
    </span>
    {#each [["map", $t("tab.map")], ["dino", $t("tab.dino")], ["settings", $t("tab.settings")], ["guide", $t("tab.guide")]] as [key, label] (key)}
      <button
        class="cursor-pointer rounded px-3 py-1 text-sm"
        style={tab === key
          ? "background: var(--color-accent); color: var(--color-bg); font-weight: 600"
          : "color: var(--color-muted)"}
        onclick={() => (tab = key as Tab)}
      >
        {label}
      </button>
    {/each}
    {#if import.meta.env.DEV}
      <button
        class="ml-auto cursor-pointer rounded border px-2 py-0.5 text-xs"
        style="border-color: var(--color-border); color: var(--color-muted)"
        onclick={simulateStep}
      >
        +300 m (dev)
      </button>
    {/if}
  </header>

  {#if updateVersion}
    <div
      class="flex shrink-0 items-center gap-3 px-3 py-2 text-sm"
      style="background: #1e3a2f; color: #a7f3d0"
    >
      {updating
        ? $t("update.installing")
        : $t("update.available", { version: updateVersion })}
      {#if !updating}
        <button
          class="cursor-pointer rounded px-2 py-0.5 font-medium"
          style="background: #34d399; color: #0b2018"
          onclick={() => void installUpdate()}
        >
          {$t("update.install")}
        </button>
        <button class="cursor-pointer underline" onclick={() => (updateVersion = null)}>
          {$t("update.later")}
        </button>
      {/if}
    </div>
  {/if}

  {#if failedHotkeys.length > 0}
    <div
      class="shrink-0 px-3 py-2 text-sm"
      style="background: #4a1a10; color: #ffb4a1"
    >
      ⚠ {$t("warn.hotkey_failed")}
      {failedHotkeys
        .map((f) => `${f.spec} (${$t(`hotkey.${f.action}` as never)})`)
        .join(", ")}
      <button
        class="ml-2 cursor-pointer underline"
        onclick={() => (failedHotkeys = [])}
      >
        {$t("btn.close")}
      </button>
    </div>
  {/if}

  {#if exclusiveFullscreen}
    <div
      class="shrink-0 px-3 py-2 text-sm"
      style="background: #4a3210; color: #ffd591"
    >
      ⚠ {$t("warn.exclusive_fullscreen")}
      <button
        class="ml-2 cursor-pointer underline"
        onclick={() => (exclusiveFullscreen = false)}
      >
        {$t("btn.close")}
      </button>
    </div>
  {/if}

  <main class="min-h-0 flex-1">
    {#if !ready}
      <div class="p-6" style="color: var(--color-muted)">…</div>
    {:else if tab === "map"}
      <!-- Only the map needs the downloaded data; the other tabs must stay
           usable during (and before) the first-run download. -->
      {#if !dataOk}
        <FirstRun oncomplete={() => void getDataStatus().then((d) => (dataStatus = d))} />
      {:else}
        <FullMap />
      {/if}
    {:else if tab === "dino"}
      <!-- Error-isolated: a failure in the IslePilot integration must never
           take down the map or any other feature. -->
      <div class="h-full overflow-y-auto">
        <svelte:boundary>
          <DinoTab />
          {#snippet failed(_error, reset)}
            <div class="mx-auto max-w-lg p-8">
              <p class="mb-3 text-sm" style="color: #ff8a80">{$t("dino.crashed")}</p>
              <button
                class="cursor-pointer rounded border px-3 py-1 text-sm"
                style="border-color: var(--color-border)"
                onclick={reset}
              >
                {$t("btn.retry")}
              </button>
            </div>
          {/snippet}
        </svelte:boundary>
      </div>
    {:else if tab === "settings"}
      <div class="h-full overflow-y-auto"><Settings /></div>
    {:else}
      <div class="h-full overflow-y-auto"><Guide /></div>
    {/if}
  </main>
</div>
