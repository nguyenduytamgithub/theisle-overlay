<script lang="ts">
  // Right-side panel: layer toggles (persisted), position status, and the
  // waypoint list with rename/delete — the CRUD UI the old app never had.
  import type { NearestWaypoint, PositionUpdate, Waypoint, WaypointPx } from "$lib/api";
  import { compassLabel, formatDistance, locale, t } from "$lib/i18n";
  import { LAYER_COLORS, LAYER_ORDER } from "$lib/theme";

  let {
    available,
    layers,
    position,
    nearest,
    waypoints,
    ontoggle,
    onrename,
    ondelete,
    onfocus,
  }: {
    available: string[];
    layers: Record<string, boolean>;
    position: PositionUpdate | null;
    nearest: NearestWaypoint | null;
    waypoints: WaypointPx[];
    ontoggle: (key: string, visible: boolean) => void;
    onrename: (id: string, name: string) => void;
    ondelete: (wp: Waypoint) => void;
    onfocus: (wp: Waypoint) => void;
  } = $props();

  let editingId = $state<string | null>(null);
  let editingName = $state("");

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
