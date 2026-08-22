<script lang="ts">
  // Bottom-right credit bar: author + GitHub/Facebook links (opened in the
  // system browser). Donations moved to their own tab (donate/Donate.svelte).
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "$lib/i18n";

  const GITHUB_URL = "https://github.com/toantranct/theisle-overlay";
  const FACEBOOK_URL = "https://www.facebook.com/satann247/";
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
  </div>
</footer>
