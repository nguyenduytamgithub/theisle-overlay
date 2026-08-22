<script lang="ts">
  // Garage (gacha) tab: one CARD per parked dino — compact 3D preview in its
  // own skin colours + info + actions. Token mode only; other states get an
  // explanation instead of dead buttons.
  //
  // Data freshness: loaded on open/login/after actions, then auto-reloaded
  // every 10 minutes while the tab is visible (never a fast poll) — the
  // Refresh button covers "now, please". Card viewers mount lazily when
  // scrolled into view so a big garage doesn't open N WebGL contexts at once.
  import { onDestroy, onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import {
    islepilotGarage,
    islepilotGaragePark,
    islepilotGarageRename,
    islepilotGarageRestore,
    islepilotGarageSell,
    islepilotState,
    listenerBag,
    onDinoLoginOk,
    type GarageDino,
    type GarageState,
  } from "$lib/api";
  import { locale, t, tNow } from "$lib/i18n";
  import DinoViewer3D from "$lib/dino3d/DinoViewer3D.svelte";
  import { prefetchSpeciesAssets } from "$lib/dino3d/model-cache";
  import { hasModel, paletteFrom } from "$lib/dino3d/registry";

  const RELOAD_MS = 10 * 60 * 1000;

  let rootEl: HTMLDivElement | undefined = $state();
  let loggedIn = $state(false);
  let authMode = $state<"token" | "legacy">("legacy");
  let garage = $state<GarageState | null>(null);
  let garageBusy = $state(false);
  let garageError = $state<string | null>(null);
  let garageNote = $state<string | null>(null);
  let renamingId = $state<string | null>(null);
  let renameInput = $state("");
  let loadedAtMs = $state<number | null>(null);
  /** Card ids whose viewer has been near the viewport at least once. */
  let seenIds = $state(new Set<string>());

  const tokenReady = $derived(loggedIn && authMode === "token");

  const timeStr = (ms: number) =>
    new Date(ms).toLocaleTimeString($locale === "vi" ? "vi-VN" : "en-US");

  onMount(() => {
    const bag = listenerBag();
    (async () => {
      const st = await islepilotState();
      loggedIn = st.loggedIn;
      authMode = st.authMode;
      if (st.loggedIn && st.authMode === "token") void loadGarage();
      await bag.add(
        onDinoLoginOk(async () => {
          const fresh = await islepilotState();
          loggedIn = fresh.loggedIn;
          authMode = fresh.authMode;
          if (fresh.loggedIn && fresh.authMode === "token") void loadGarage();
        }),
      );
    })();
    return () => bag.dispose();
  });

  // Slow auto-refresh: only while the tab is actually on screen (this
  // component stays mounted-but-hidden after the first visit).
  const reloadTimer = setInterval(() => {
    const visible = rootEl && rootEl.offsetParent !== null;
    const stale = loadedAtMs !== null && Date.now() - loadedAtMs >= RELOAD_MS;
    if (visible && stale && tokenReady && !garageBusy) void loadGarage();
  }, 60_000);
  onDestroy(() => clearInterval(reloadTimer));

  async function loadGarage() {
    try {
      garage = await islepilotGarage();
      garageError = null;
      loadedAtMs = Date.now();
      // Warm the disk cache for every parked species in the background so
      // each card's model opens instantly.
      void prefetchSpeciesAssets(garage.dinos.map((d) => dinoSpecies(d)));
    } catch (e) {
      garageError = String(e);
      garage = null;
    }
  }

  async function garageDo(fn: () => Promise<unknown>, confirmMsg?: string) {
    if (confirmMsg) {
      const yes = await ask(confirmMsg, { title: tNow("garage.title"), kind: "warning" });
      if (!yes) return;
    }
    garageBusy = true;
    garageError = null;
    garageNote = null;
    try {
      await fn();
      garageNote = tNow("garage.done");
      await loadGarage();
    } catch (e) {
      garageError = String(e);
    } finally {
      garageBusy = false;
    }
  }

  async function submitRename(id: string) {
    const name = renameInput.trim();
    renamingId = null;
    if (!name) return;
    await garageDo(() => islepilotGarageRename(id, name));
  }

  // The garage record shape is the backend's own — read it defensively so a
  // field rename over there degrades to "—" instead of crashing the tab.
  const dStr = (d: GarageDino, keys: string[]): string | null => {
    for (const k of keys) {
      const v = d[k];
      if (typeof v === "string" && v) return v;
    }
    return null;
  };
  const dNum = (d: GarageDino, keys: string[]): number | null => {
    for (const k of keys) {
      const v = d[k];
      if (typeof v === "number" && isFinite(v)) return v;
    }
    return null;
  };
  const dinoId = (d: GarageDino): string | null =>
    dStr(d, ["id", "_id", "dinoId"]) ?? (typeof d.id === "number" ? String(d.id) : null);
  const dinoName = (d: GarageDino): string =>
    dStr(d, ["name", "label"]) ?? dStr(d, ["species", "dino"]) ?? "?";
  const dinoSpecies = (d: GarageDino): string | null => dStr(d, ["species", "dino"]);
  const dinoGrowthPct = (d: GarageDino): number | null => {
    const g = dNum(d, ["growth", "growthPct"]);
    if (g === null) return null;
    return g <= 1.5 ? g * 100 : g;
  };
  const dinoPalette = (d: GarageDino) =>
    paletteFrom(d.palette ?? (d as Record<string, unknown>).skin ?? null);

  /** Svelte action: mark the card id "seen" once it nears the viewport, so
   * its 3D viewer mounts lazily (and stays — the caches make it cheap). */
  function inview(node: HTMLElement, id: string | null) {
    if (!id) return {};
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          seenIds.add(id);
          seenIds = new Set(seenIds);
          io.disconnect();
        }
      },
      { rootMargin: "150px" },
    );
    io.observe(node);
    return { destroy: () => io.disconnect() };
  }
</script>

<div class="mx-auto max-w-4xl space-y-4 p-6" bind:this={rootEl}>
  <div class="flex items-center justify-between">
    <h2 class="text-lg font-semibold" style="color: var(--color-accent)">
      {$t("garage.title")}
      {#if garage}
        <span class="text-sm font-normal" style="color: var(--color-muted)">
          ({garage.dinos.length})
        </span>
      {/if}
    </h2>
    {#if tokenReady}
      <div class="flex gap-2">
        <button
          class="cursor-pointer rounded border px-3 py-1 text-sm disabled:opacity-50"
          style="border-color: var(--color-border)"
          disabled={garageBusy}
          onclick={() => void loadGarage()}
        >
          {$t("garage.refresh")}
        </button>
        <button
          class="cursor-pointer rounded px-3 py-1 text-sm font-medium disabled:opacity-50"
          style="background: var(--color-accent); color: var(--color-bg)"
          disabled={garageBusy}
          onclick={() => void garageDo(() => islepilotGaragePark())}
        >
          {$t("garage.park")}
        </button>
      </div>
    {/if}
  </div>

  {#if !tokenReady}
    <!-- Not usable: explain why instead of showing dead buttons. -->
    <section
      class="rounded border p-6 text-center"
      style="border-color: var(--color-border); background: var(--color-panel)"
    >
      <p class="text-sm" style="color: var(--color-muted)">{$t("garage.need_token")}</p>
    </section>
  {:else}
    {#if loadedAtMs !== null}
      <p class="text-xs" style="color: var(--color-muted)">
        {$t("garage.updated", { time: timeStr(loadedAtMs) })}
      </p>
    {/if}

    {#if garageBusy}
      <p class="text-sm" style="color: #ffd591">{$t("garage.busy")}</p>
    {/if}
    {#if garageError}
      <section
        class="rounded border p-4"
        style="border-color: var(--color-border); background: var(--color-panel)"
      >
        <p class="text-sm" style="color: #ffd591">{$t("garage.unsupported")}</p>
        <p class="mt-1 font-mono text-xs" style="color: var(--color-muted)">{garageError}</p>
      </section>
    {/if}
    {#if garageNote && !garageBusy && !garageError}
      <p class="text-sm" style="color: #72d653">{garageNote}</p>
    {/if}

    {#if garage}
      {#if garage.dinos.length === 0}
        <section
          class="rounded border p-6 text-center"
          style="border-color: var(--color-border); background: var(--color-panel)"
        >
          <p class="text-sm" style="color: var(--color-muted)">{$t("garage.empty")}</p>
        </section>
      {:else}
        <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
          {#each garage.dinos as dino, i (dinoId(dino) ?? i)}
            {@const id = dinoId(dino)}
            {@const sp = dinoSpecies(dino)}
            {@const growth = dinoGrowthPct(dino)}
            <div
              class="overflow-hidden rounded border"
              style="border-color: var(--color-border); background: var(--color-panel)"
              use:inview={id}
            >
              <!-- Compact 3D preview (lazy: mounts when the card scrolls in) -->
              {#if sp && hasModel(sp)}
                {#if id && seenIds.has(id)}
                  <DinoViewer3D species={sp} palette={dinoPalette(dino)} height={190} />
                {:else}
                  <div style="height: 190px"></div>
                {/if}
              {:else}
                <div
                  class="flex items-center justify-center text-xs"
                  style="height: 60px; color: var(--color-muted)"
                >
                  {$t("dino3d.no_model")}
                </div>
              {/if}

              <div class="space-y-2 p-3">
                <div class="flex flex-wrap items-center gap-2">
                  <span class="text-sm font-semibold">{dinoName(dino)}</span>
                  {#if sp && sp !== dinoName(dino)}
                    <span class="text-xs" style="color: var(--color-muted)">{sp}</span>
                  {/if}
                  {#if growth !== null}
                    <span class="ml-auto font-mono text-xs" style="color: var(--color-muted)">
                      {$t("dino.growth")} {growth.toFixed(0)}%
                    </span>
                  {/if}
                </div>
                {#if id}
                  <div class="flex gap-1.5">
                    <button
                      class="cursor-pointer rounded px-2 py-0.5 text-xs font-medium disabled:opacity-50"
                      style="background: var(--color-accent); color: var(--color-bg)"
                      disabled={garageBusy}
                      onclick={() =>
                        void garageDo(
                          () => islepilotGarageRestore(id),
                          tNow("garage.confirm_restore", { name: dinoName(dino) }),
                        )}
                    >
                      {$t("garage.restore")}
                    </button>
                    <button
                      class="cursor-pointer rounded border px-2 py-0.5 text-xs disabled:opacity-50"
                      style="border-color: var(--color-border)"
                      disabled={garageBusy}
                      onclick={() => {
                        renamingId = renamingId === id ? null : id;
                        renameInput = dinoName(dino);
                      }}
                    >
                      {$t("garage.rename")}
                    </button>
                    {#if garage.sellingEnabled}
                      <button
                        class="cursor-pointer rounded border px-2 py-0.5 text-xs disabled:opacity-50"
                        style="border-color: #e2664a; color: #e2664a"
                        disabled={garageBusy}
                        onclick={() =>
                          void garageDo(
                            () => islepilotGarageSell(id),
                            tNow("garage.confirm_sell", { name: dinoName(dino) }),
                          )}
                      >
                        {$t("garage.sell")}
                      </button>
                    {/if}
                  </div>
                {/if}
                {#if id && renamingId === id}
                  <div class="flex gap-2">
                    <input
                      class="w-full rounded border px-2 py-1 text-xs"
                      style="border-color: var(--color-border); background: var(--color-bg); color: var(--color-text)"
                      placeholder={$t("garage.rename_prompt")}
                      bind:value={renameInput}
                      onkeydown={(e) => {
                        if (e.key === "Enter") void submitRename(id);
                        if (e.key === "Escape") renamingId = null;
                      }}
                    />
                    <button
                      class="cursor-pointer rounded border px-2 py-0.5 text-xs"
                      style="border-color: var(--color-border)"
                      onclick={() => void submitRename(id)}
                    >
                      {$t("btn.save")}
                    </button>
                  </div>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  {/if}
</div>
