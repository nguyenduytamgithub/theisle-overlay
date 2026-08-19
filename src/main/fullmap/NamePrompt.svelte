<script lang="ts">
  // Small in-app prompt (window.prompt is unreliable inside WebView2).
  import { t } from "$lib/i18n";

  let {
    open,
    title,
    label,
    onconfirm,
    oncancel,
  }: {
    open: boolean;
    title: string;
    label: string;
    onconfirm: (name: string) => void;
    oncancel: () => void;
  } = $props();

  let name = $state("");

  $effect(() => {
    if (open) name = "";
  });
</script>

{#if open}
  <div class="fixed inset-0 z-[1000] flex items-center justify-center bg-black/50">
    <div
      class="w-72 rounded-lg border p-4 shadow-xl"
      style="background: var(--color-panel); border-color: var(--color-border)"
    >
      <h3 class="mb-2 font-semibold" style="color: var(--color-accent)">{title}</h3>
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
