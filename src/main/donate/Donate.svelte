<script lang="ts">
  // Donate tab: the VietQR + bank info that used to live in the Footer
  // popup, promoted to its own tab.
  import { t } from "$lib/i18n";
  import qrDonate from "../../assets/qr_donate.png";

  const BANK_ACCOUNT = "8866886767";
  const BANK_INFO = "Techcombank · TRAN QUOC TOAN";

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

<div class="mx-auto max-w-md p-8 text-center">
  <h2 class="mb-1 text-xl font-semibold" style="color: var(--color-accent)">
    {$t("donate.title")}
  </h2>
  <p class="mb-5 text-sm" style="color: var(--color-muted)">{$t("donate.hint")}</p>
  <img src={qrDonate} alt="VietQR" class="mx-auto mb-5 w-72 rounded-lg bg-white p-2" />
  <div class="mb-1 font-mono text-2xl font-semibold tracking-wider">
    {BANK_ACCOUNT}
  </div>
  <div class="mb-4 text-sm" style="color: var(--color-muted)">{BANK_INFO}</div>
  <button
    class="cursor-pointer rounded px-4 py-2 text-sm font-medium"
    style="background: {copied ? '#72d653' : 'var(--color-accent)'}; color: var(--color-bg)"
    onclick={() => void copyStk()}
  >
    {copied ? $t("donate.copied") : $t("donate.copy_stk")}
  </button>
  <p class="mt-6 text-sm" style="color: var(--color-muted)">{$t("donate.thanks")}</p>
</div>
