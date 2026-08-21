<script lang="ts">
  // First-run download screen. Explains WHY data is fetched instead of
  // bundled (licensing), shows per-file progress, and fails soft: a working
  // basemap without POI data is a valid outcome.
  import { onMount } from "svelte";
  import {
    listenerBag,
    onFetchFinished,
    onFetchProgress,
    startFetchData,
    type FetchFinished,
    type FetchProgress,
  } from "$lib/api";
  import { t } from "$lib/i18n";

  let { oncomplete }: { oncomplete: () => void } = $props();

  type Phase = "idle" | "running" | "done" | "partial" | "failed";
  let phase = $state<Phase>("idle");
  let progress = $state<FetchProgress[]>([]);
  let doneTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    const bag = listenerBag();
    (async () => {
      await bag.add(
        onFetchProgress((p) => {
          const idx = progress.findIndex((x) => x.file === p.file);
          if (idx >= 0) progress[idx] = p;
          else progress = [...progress, p];
        }),
      );
      await bag.add(
        onFetchFinished((f: FetchFinished) => {
          if (f.ok) {
            phase = "done";
            doneTimer = setTimeout(oncomplete, 800);
          } else if (f.basemapOk) {
            phase = "partial";
          } else {
            phase = "failed";
          }
        }),
      );
    })();
    return () => {
      clearTimeout(doneTimer);
      bag.dispose();
    };
  });

  function start() {
    progress = [];
    phase = "running";
    void startFetchData(false);
  }

  const ICONS: Record<FetchProgress["status"], string> = {
    downloading: "⏳",
    done: "✓",
    skipped: "•",
    error: "✗",
  };
</script>

<div class="mx-auto max-w-lg p-8">
  <h2 class="mb-3 text-lg font-semibold" style="color: var(--color-accent)">
    {$t("firstrun.title")}
  </h2>
  <p class="mb-4 text-sm leading-relaxed">{$t("firstrun.explain")}</p>
  <p class="mb-4 text-xs leading-relaxed" style="color: var(--color-muted)">
    {$t("credits.body")}
  </p>

  {#if phase === "idle"}
    <button
      class="cursor-pointer rounded px-4 py-2 font-medium"
      style="background: var(--color-accent); color: var(--color-bg)"
      onclick={start}
    >
      {$t("firstrun.start")}
    </button>
  {:else}
    <ul class="mb-4 space-y-1 font-mono text-sm">
      {#each progress as p (p.file)}
        <li class="flex items-center gap-2">
          <span
            style="color: {p.status === 'error'
              ? '#ff8a80'
              : p.status === 'done'
                ? '#72d653'
                : 'var(--color-muted)'}"
          >
            {ICONS[p.status]}
          </span>
          <span>{p.file}</span>
          {#if p.error}
            <span class="truncate text-xs" style="color: #ff8a80">{p.error}</span>
          {/if}
        </li>
      {/each}
    </ul>

    {#if phase === "running"}
      <p class="text-sm" style="color: var(--color-muted)">{$t("firstrun.downloading")}</p>
    {:else if phase === "done"}
      <p class="text-sm" style="color: #72d653">{$t("firstrun.done")}</p>
    {:else if phase === "partial"}
      <p class="mb-3 text-sm" style="color: #ffd591">{$t("firstrun.partial")}</p>
      <div class="flex gap-2">
        <button
          class="cursor-pointer rounded px-3 py-1.5 text-sm font-medium"
          style="background: var(--color-accent); color: var(--color-bg)"
          onclick={oncomplete}
        >
          {$t("firstrun.continue")}
        </button>
        <button
          class="cursor-pointer rounded border px-3 py-1.5 text-sm"
          style="border-color: var(--color-border)"
          onclick={start}
        >
          {$t("firstrun.retry")}
        </button>
      </div>
    {:else}
      <p class="mb-3 text-sm" style="color: #ff8a80">{$t("firstrun.failed")}</p>
      <button
        class="cursor-pointer rounded px-3 py-1.5 text-sm font-medium"
        style="background: var(--color-accent); color: var(--color-bg)"
        onclick={start}
      >
        {$t("firstrun.retry")}
      </button>
    {/if}
  {/if}
</div>
