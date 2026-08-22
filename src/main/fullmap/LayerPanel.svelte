<script lang="ts">
  // Right-side panel: layer toggles (persisted), position status, and the
  // waypoint list with rename/delete — the CRUD UI the old app never had.
  import type { NearestWaypoint, PositionUpdate, Waypoint, WaypointPx } from "$lib/api";
  import { compassLabel, formatDistance, locale, t } from "$lib/i18n";
  import { LAYER_COLORS, LAYER_ORDER } from "$lib/theme";

  let {
    available,
    layers,
    zoneLabels,
    position,
    nearest,
    waypoints,
    places,
    islepilotNote = null,
    ontoggle,
    ontogglezonelabels,
    onrename,
    ondelete,
    onfocus,
    oncleartrail,
    onsetcolor,
    onlocate,
    onsearchcoords,
  }: {
    available: string[];
    layers: Record<string, boolean>;
    zoneLabels: boolean;
    position: PositionUpdate | null;
    nearest: NearestWaypoint | null;
    waypoints: WaypointPx[];
    places: { label: string; px: number; py: number; kind: string }[];
    /** Why IslePilot server POIs are unavailable (already localized). */
    islepilotNote?: string | null;
    ontoggle: (key: string, visible: boolean) => void;
    ontogglezonelabels: (visible: boolean) => void;
    onrename: (id: string, name: string) => void;
    ondelete: (wp: Waypoint) => void;
    onfocus: (wp: Waypoint) => void;
    oncleartrail: () => void;
    onsetcolor: (wp: Waypoint, color: string | null) => void;
    onlocate: (px: number, py: number) => void;
    onsearchcoords: (text: string) => Promise<boolean>;
  } = $props();

  let editingId = $state<string | null>(null);
  let editingName = $state("");

  // --- search ---------------------------------------------------------------
  let query = $state("");
  let coordsFailed = $state(false);

  const looksLikeCoords = (q: string) => /\d[\d.,\s−-]*\d/.test(q);

  const results = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return [];
    return [
      ...waypoints.map((w) => ({ label: w.name, px: w.px, py: w.py, kind: "wp" })),
      ...places,
    ]
      .filter((p) => p.label.toLowerCase().includes(q))
      .slice(0, 6);
  });

  function pickResult(r: { px: number; py: number }) {
    onlocate(r.px, r.py);
    query = "";
    coordsFailed = false;
  }

  async function tryCoords() {
    coordsFailed = !(await onsearchcoords(query));
    if (!coordsFailed) query = "";
  }

  function onSearchKey(e: KeyboardEvent) {
    coordsFailed = false;
    if (e.key === "Escape") {
      query = "";
      return;
    }
    if (e.key !== "Enter") return;
    if (results.length > 0) pickResult(results[0]);
    else if (looksLikeCoords(query)) void tryCoords();
  }

  const kindColor = (kind: string) =>
    kind === "wp" ? "#4fc3f7" : (LAYER_COLORS[kind] ?? "#e8a33d");

  // --- waypoint colours ------------------------------------------------------
  const WP_PALETTE = ["#4fc3f7", "#ef5350", "#ffa726", "#ffee58", "#66bb6a", "#ab47bc", "#eceff1"];

  function cycleColor(wp: WaypointPx) {
    const i = WP_PALETTE.indexOf(wp.color ?? WP_PALETTE[0]);
    onsetcolor(wp, WP_PALETTE[(i + 1) % WP_PALETTE.length]);
  }

  const layerKey = (key: string) => `layer.${key}` as Parameters<typeof $t>[0];

  function startRename(wp: Waypoint) {
    editingId = wp.id;
    editingName = wp.name;
  }

  function commitRename() {
    if (editingId && editingName.trim()) onrename(editingId, editingName.trim());
    editingId = null;
  }

  const fmt = (n: number) =>
    Math.round(n).toLocaleString($locale === "vi" ? "vi-VN" : "en-US");
</script>

<aside
  class="flex w-56 shrink-0 flex-col gap-3 overflow-y-auto p-3"
  style="background: var(--color-panel); border-left: 1px solid var(--color-border)"
>
  <section>
    <input
      class="w-full rounded border px-2 py-1 text-sm"
      style="border-color: var(--color-border); background: var(--color-bg); color: var(--color-text)"
      placeholder={$t("search.placeholder")}
      bind:value={query}
      onkeydown={onSearchKey}
    />
    {#if query.trim()}
      <ul class="mt-1 space-y-0.5">
        {#each results as r (r.kind + r.label)}
          <li>
            <button
              class="flex w-full cursor-pointer items-center gap-2 rounded px-1.5 py-1 text-left text-sm hover:underline"
              onclick={() => pickResult(r)}
            >
              <span
                class="inline-block size-2 shrink-0 rounded-full"
                style="background: {kindColor(r.kind)}"
              ></span>
              <span class="truncate">{r.label}</span>
            </button>
          </li>
        {/each}
        {#if looksLikeCoords(query)}
          <li>
            <button
              class="w-full cursor-pointer rounded px-1.5 py-1 text-left text-sm hover:underline"
              style="color: var(--color-accent)"
              onclick={() => void tryCoords()}
            >
              → {$t("search.goto_coords")}
            </button>
          </li>
        {/if}
        {#if results.length === 0 && !looksLikeCoords(query)}
          <li class="px-1.5 py-1 text-xs" style="color: var(--color-muted)">
            {$t("search.no_results")}
          </li>
        {/if}
        {#if coordsFailed}
          <li class="px-1.5 py-1 text-xs" style="color: #ff8a80">
            {$t("search.coords_failed")}
          </li>
        {/if}
      </ul>
    {/if}
  </section>

  <section>
    <h2 class="mb-1 text-sm font-semibold" style="color: var(--color-accent)">
      {$t("layers.title")}
    </h2>
    {#each LAYER_ORDER as key (key)}
      {#if available.includes(key)}
        <label class="flex cursor-pointer items-center gap-2 py-1 text-sm">
          <input
            type="checkbox"
            class="size-3.5 accent-current"
            style="color: {LAYER_COLORS[key]}"
            checked={layers[key] ?? true}
            onchange={(e) => ontoggle(key, e.currentTarget.checked)}
          />
          <span
            class="inline-block size-2.5 rounded-full"
            style="background: {LAYER_COLORS[key]}"
          ></span>
          {$t(layerKey(key))}
        </label>
      {/if}
    {/each}
    {#if islepilotNote}
      <p class="py-1 text-xs" style="color: var(--color-muted)">{islepilotNote}</p>
    {/if}
    <label
      class="mt-1 flex cursor-pointer items-center gap-2 border-t pt-1.5 text-sm"
      style="border-color: var(--color-border)"
    >
      <input
        type="checkbox"
        class="size-3.5"
        checked={zoneLabels}
        onchange={(e) => ontogglezonelabels(e.currentTarget.checked)}
      />
      {$t("layers.zone_labels")}
    </label>
  </section>

  <section>
    <h2 class="mb-1 text-sm font-semibold" style="color: var(--color-accent)">
      {$t("trail.title")}
    </h2>
    <button
      class="cursor-pointer rounded border px-2 py-1 text-xs"
      style="border-color: var(--color-border)"
      onclick={() => oncleartrail()}
    >
      {$t("trail.clear")}
    </button>
    <p class="mt-1 text-xs leading-snug" style="color: var(--color-muted)">
      {$t("trail.clear_hint")}
    </p>
  </section>

  <section class="text-sm" style="color: var(--color-muted)">
    {#if position}
      <div class="font-mono" style="color: var(--color-text)">
        X {fmt(position.xCm)}<br />
        Y {fmt(position.yCm)}
      </div>
      {#if !position.inBounds}
        <div>{$t("pos.off_map")}</div>
      {/if}
      {#if position.headingDeg !== null}
        <div>
          {compassLabel($locale, position.compassKey)}
          {Math.round(position.headingDeg)}°
        </div>
      {:else}
        <div>{$t("heading.unknown")}</div>
      {/if}
      {#if nearest}
        <div class="mt-2 border-t pt-2" style="border-color: var(--color-border)">
          <div style="color: var(--color-text)">{nearest.name}</div>
          <div>
            {$t("wp.distance", {
              dir: compassLabel($locale, nearest.compassKey),
              dist: formatDistance($locale, nearest.distanceM),
            })}
          </div>
        </div>
      {/if}
    {:else}
      <div>{$t("pos.none")}</div>
      <div class="mt-1 text-xs">{$t("pos.hint")}</div>
    {/if}
  </section>

  <section class="min-h-0 flex-1">
    <h2 class="mb-1 text-sm font-semibold" style="color: var(--color-accent)">
      {$t("wp.title")}
    </h2>
    {#if waypoints.length === 0}
      <p class="text-xs" style="color: var(--color-muted)">{$t("wp.empty")}</p>
    {/if}
    <ul class="space-y-1">
      {#each waypoints as wp (wp.id)}
        <li
          class="rounded border p-1.5 text-sm"
          style="border-color: var(--color-border); background: var(--color-bg)"
        >
          {#if editingId === wp.id}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="w-full rounded border px-1 py-0.5 text-sm"
              style="border-color: var(--color-accent); background: var(--color-panel); color: var(--color-text)"
              bind:value={editingName}
              autofocus
              onkeydown={(e) => {
                if (e.key === "Enter") commitRename();
                if (e.key === "Escape") editingId = null;
              }}
              onblur={commitRename}
            />
          {:else}
            <div class="flex items-center gap-1">
              <button
                class="min-w-0 flex-1 cursor-pointer truncate text-left hover:underline"
                title={wp.name}
                onclick={() => onfocus(wp)}
              >
                {wp.name}
              </button>
              <button
                class="size-3.5 shrink-0 cursor-pointer rounded-full border opacity-80 hover:opacity-100"
                style="background: {wp.color ?? '#4fc3f7'}; border-color: rgba(255,255,255,0.55)"
                title={$t("wp.color")}
                aria-label={$t("wp.color")}
                onclick={() => cycleColor(wp)}
              ></button>
              <button
                class="shrink-0 cursor-pointer px-1 text-xs opacity-70 hover:opacity-100"
                title={$t("wp.rename")}
                onclick={() => startRename(wp)}
              >
                ✎
              </button>
              <button
                class="shrink-0 cursor-pointer px-1 text-xs opacity-70 hover:opacity-100"
                title={$t("wp.remove")}
                onclick={() => ondelete(wp)}
              >
                ✕
              </button>
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  </section>
</aside>
