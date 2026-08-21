<script lang="ts">
  // Bottom-right credit bar: author, GitHub/Facebook links (opened in the
  // system browser), and a donate popup with the VietQR image.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "$lib/i18n";
  import qrDonate from "../assets/qr_donate.png";

  const GITHUB_URL = "https://github.com/toantranct/theisle-overlay";
  const FACEBOOK_URL = "https://www.facebook.com/satann247/";
  const BANK_ACCOUNT = "8866886767";
  const BANK_INFO = "Techcombank · TRAN QUOC TOAN";

  let donateOpen = $state(false);
  let copied = $state(false);

  async function copyStk() {
    try {
      await navigator.clipboard.writeText(BANK_ACCOUNT);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch {
      // Clipboard API unavailable: leave the number visible to copy by hand.
    }
  }
</script>

<footer
  class="flex shrink-0 items-center justify-between gap-3 border-t px-3 py-1 text-xs"
  style="border-color: var(--color-border); background: var(--color-panel); color: var(--color-muted)"
>
  <span>v{__APP_VERSION__} · {$t("footer.reload_hint")}</span>
  <div class="flex items-center gap-3">
    <span>{$t("footer.developed_by")} <span style="color: var(--color-text)">Trần Quốc Toản</span></span>
    <button
      class="cursor-pointer underline-offset-2 hover:underline"
      onclick={() => void openUrl(GITHUB_URL)}
    >
      GitHub
    </button>
    <button
      class="cursor-pointer underline-offset-2 hover:underline"
      onclick={() => void openUrl(FACEBOOK_URL)}
    >
      Facebook
    </button>
    <button
      class="cursor-pointer rounded px-2 py-0.5 font-medium"
      style="background: var(--color-accent); color: var(--color-bg)"
      onclick={() => (donateOpen = true)}
    >
      ❤ {$t("footer.donate")}
    </button>
  </div>
</footer>

{#if donateOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-[1100] flex items-center justify-center bg-black/60"
    onclick={(e) => {
      if (e.target === e.currentTarget) donateOpen = false;
    }}
  >
    <div
      class="w-80 rounded-lg border p-4 text-center shadow-xl"
      style="background: var(--color-panel); border-color: var(--color-border)"
    >
      <h3 class="mb-1 text-base font-semibold" style="color: var(--color-accent)">
        {$t("donate.title")}
      </h3>
      <p class="mb-3 text-xs" style="color: var(--color-muted)">{$t("donate.hint")}</p>
      <img
        src={qrDonate}
        alt="VietQR"
        class="mx-auto mb-3 w-56 rounded bg-white"
      />
      <div class="mb-1 font-mono text-lg font-semibold tracking-wider">
        {BANK_ACCOUNT}
      </div>
      <div class="mb-3 text-xs" style="color: var(--color-muted)">{BANK_INFO}</div>
      <div class="flex justify-center gap-2">
        <button
          class="cursor-pointer rounded px-3 py-1.5 text-sm font-medium"
          style="background: {copied ? '#72d653' : 'var(--color-accent)'}; color: var(--color-bg)"
          onclick={() => void copyStk()}
        >
          {copied ? $t("donate.copied") : $t("donate.copy_stk")}
        </button>
        <button
          class="cursor-pointer rounded border px-3 py-1.5 text-sm"
          style="border-color: var(--color-border)"
          onclick={() => (donateOpen = false)}
        >
          {$t("btn.close")}
        </button>
      </div>
      <p class="mt-3 text-xs" style="color: var(--color-muted)">{$t("donate.thanks")}</p>
    </div>
  </div>
{/if}
