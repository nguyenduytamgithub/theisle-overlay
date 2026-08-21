<script lang="ts">
  // Small in-app prompt (window.prompt is unreliable inside WebView2).
  import { t } from "$lib/i18n";

  let {
    open,
    title,
    label,
    presets = [],
    onconfirm,
    oncancel,
  }: {
    open: boolean;
    title: string;
    label: string;
    /** Optional emoji shortcuts prepended to the name (💀 chỗ chết, …). */
    presets?: string[];
    onconfirm: (name: string) => void;
    oncancel: () => void;
  } = $props();

  let name = $state("");

  $effect(() => {
    if (open) name = "";
  });

  function applyPreset(p: string) {
    // Toggle-style: replace an existing leading preset instead of stacking.
    const stripped = presets.reduce((n, e) => (n.startsWith(e) ? n.slice(e.length).trimStart() : n), name);
    name = `${p} ${stripped}`.trimEnd();
    document.getElementById("name-prompt-input")?.focus();
  }
</script>

{#if open}
  <div class="fixed inset-0 z-[1000] flex items-center justify-center bg-black/50">
    <div
      class="w-72 rounded-lg border p-4 shadow-xl"
      style="background: var(--color-panel); border-color: var(--color-border)"
    >
      <h3 class="mb-2 font-semibold" style="color: var(--color-accent)">{title}</h3>
      {#if presets.length > 0}
        <div class="mb-2 flex gap-1">
          {#each presets as p (p)}
            <button
              class="cursor-pointer rounded border px-1.5 py-0.5 text-base leading-none"
              style="border-color: var(--color-border)"
              onclick={() => applyPreset(p)}
            >
              {p}
            </button>
          {/each}
        </div>
      {/if}
      <label class="mb-1 block text-sm" for="name-prompt-input">{label}</label>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        id="name-prompt-input"
        class="mb-3 w-full rounded border px-2 py-1"
        style="border-color: var(--color-border); background: var(--color-bg); color: var(--color-text)"
        bind:value={name}
        autofocus
        onkeydown={(e) => {
          if (e.key === "Enter") onconfirm(name.trim());
          if (e.key === "Escape") oncancel();
        }}
      />
      <div class="flex justify-end gap-2">
        <button
          class="cursor-pointer rounded border px-3 py-1 text-sm"
          style="border-color: var(--color-border)"
          onclick={oncancel}
        >
          {$t("btn.cancel")}
        </button>
        <button
          class="cursor-pointer rounded px-3 py-1 text-sm font-medium"
          style="background: var(--color-accent); color: var(--color-bg)"
          onclick={() => onconfirm(name.trim())}
        >
          {$t("btn.ok")}
        </button>
      </div>
    </div>
  </div>
{/if}
