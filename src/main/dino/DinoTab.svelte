<script lang="ts">
  // "Your dino" tab: IslePilot login + live stats + Prime progress.
  //
  // Two auth modes: TOKEN (primary — one Steam login against islepilot.eu,
  // works on every IslePilot server) and LEGACY (fallback — per-server URL +
  // cookie, the original flow, collapsed under <details>).
  import { onMount } from "svelte";
  import {
    getSettings,
    islepilotApply,
    islepilotCancelLogin,
    islepilotLogin,
    islepilotLogout,
    islepilotSetCookie,
    islepilotSetToken,
    islepilotState,
    islepilotTokenLogin,
    listenerBag,
    onDinoAuthExpired,
    onDinoLoginFailed,
    onDinoLoginOk,
    onDinoUpdate,
    patchSettings,
    type DinoStatBar,
    type DinoUpdate,
    type Settings,
  } from "$lib/api";
  import { locale, t } from "$lib/i18n";


  let settings = $state<Settings | null>(null);
  let loggedIn = $state(false);
  let authMode = $state<"token" | "legacy">("legacy");
  // Login setup is bulky; once signed in it collapses so the stats are
  // visible without scrolling. The gear button reopens it.
  let serverOpen = $state(true);
  let update = $state<DinoUpdate | null>(null);
  let loginBusy = $state(false);
  let loginError = $state(false);
  let authExpired = $state(false);
  let domainInput = $state("");
  let cookieInput = $state("");
  let cookieBusy = $state(false);
  let cookieError = $state(false);
  let tokenInput = $state("");
  let tokenBusy = $state(false);
  let tokenError = $state(false);

  async function refreshState() {
    const st = await islepilotState();
    loggedIn = st.loggedIn;
    authMode = st.authMode;
    update = st.lastUpdate ?? update;
  }

  onMount(() => {
    const bag = listenerBag();
    (async () => {
      settings = await getSettings();
      domainInput = settings.islepilot.domain;
      const st = await islepilotState();
      loggedIn = st.loggedIn;
      authMode = st.authMode;
      serverOpen = !st.loggedIn;
      update = st.lastUpdate;
      await bag.add(
        onDinoUpdate((u) => {
          update = u;
          authExpired = false;
        }),
      );
      await bag.add(
        onDinoLoginOk(async () => {
          loginBusy = false;
          loginError = false;
          authExpired = false;
          serverOpen = false;
          settings = await getSettings();
          await refreshState();
        }),
      );
      await bag.add(
        onDinoLoginFailed(() => {
          loginBusy = false;
          loginError = true;
        }),
      );
      await bag.add(
        onDinoAuthExpired(() => {
          authExpired = true;
          serverOpen = true; // re-login lives in the collapsed section
        }),
      );
    })();
    return () => bag.dispose();
  });

  async function patch(p: object, reapply = false) {
    settings = await patchSettings(p);
    if (reapply) await islepilotApply();
  }

  async function tokenLogin() {
    loginBusy = true;
    loginError = false;
    try {
      await islepilotTokenLogin();
    } catch {
      loginBusy = false;
      loginError = true;
    }
  }

  async function saveToken() {
    tokenBusy = true;
    tokenError = false;
    try {
      await islepilotSetToken(tokenInput);
      tokenInput = "";
    } catch {
      tokenError = true;
    } finally {
      tokenBusy = false;
    }
  }

  async function login() {
    loginBusy = true;
    loginError = false;
    try {
      await islepilotLogin(domainInput.trim());
    } catch {
      loginBusy = false;
      loginError = true;
    }
  }

  async function logout() {
    await islepilotLogout();
    loggedIn = false;
    serverOpen = true;
    update = null;
    settings = await getSettings();
  }

  async function cancelLogin() {
    await islepilotCancelLogin();
    loginBusy = false;
  }

  async function saveCookie() {
    cookieBusy = true;
    cookieError = false;
    try {
      await islepilotSetCookie(domainInput.trim(), cookieInput);
      cookieInput = "";
    } catch {
      cookieError = true;
    } finally {
      cookieBusy = false;
    }
  }

  const pct = (bar: DinoStatBar | null): number | null =>
    bar && bar.current !== null && bar.max ? (bar.current / bar.max) * 100 : null;

  const hpColor = (p: number | null) =>
    p === null ? "#4aa8d8" : p > 50 ? "#72d653" : p > 25 ? "#e8a33d" : "#e2664a";

  const timeStr = (ms: number) =>
    new Date(ms).toLocaleTimeString($locale === "vi" ? "vi-VN" : "en-US");

  const player = $derived(update?.player ?? null);
  /** Server live-map capability: true / false / null while unknown. */
  const liveMap = $derived(update?.liveMapAvailable ?? null);
  const nutrition = $derived(player?.nutrition ?? null);
</script>

{#if settings}
  <div class="mx-auto max-w-2xl space-y-5 p-6">
    <section>
      <div class="flex items-center justify-between">
        <h2 class="text-lg font-semibold" style="color: var(--color-accent)">
          {$t("dino.title")}
        </h2>
        <button
          class="cursor-pointer rounded border px-2 py-1 text-xs"
          style={serverOpen
            ? "border-color: var(--color-accent); color: var(--color-accent)"
            : "border-color: var(--color-border); color: var(--color-muted)"}
          onclick={() => (serverOpen = !serverOpen)}
        >
          ⚙ {$t("dino.server_settings")}
        </button>
      </div>
      {#if serverOpen}
        <p class="mt-1 text-sm" style="color: var(--color-muted)">{$t("dino.explain")}</p>
        <p class="mt-2 text-xs" style="color: #ffd591">{$t("dino.rules_note")}</p>
      {/if}
    </section>

    <!-- Login (collapsed once signed in) -->
    {#if serverOpen}
    <section
      class="rounded border p-3"
      style="border-color: var(--color-border); background: var(--color-panel)"
    >
      <!-- Primary: token mode — one Steam login for every server -->
      <div class="mb-1 text-sm font-semibold" style="color: var(--color-accent)">
        {$t("dino.token_login")}
      </div>
      <p class="mb-2 text-xs leading-relaxed" style="color: var(--color-muted)">
        {$t("dino.token_login_hint")}
      </p>
      {#if authExpired}
        <p class="mb-2 text-sm" style="color: #ff8a80">{$t("dino.auth_expired")}</p>
      {/if}
      {#if loginError}
        <p class="mb-2 text-sm" style="color: #ff8a80">{$t("dino.login_failed")}</p>
      {/if}
      <div class="flex items-center gap-3">
        {#if loggedIn && authMode === "token" && !authExpired}
          <span class="text-sm" style="color: #72d653">✓ {$t("dino.logged_in")}</span>
          <button
            class="cursor-pointer rounded border px-3 py-1 text-sm"
            style="border-color: var(--color-border)"
            onclick={() => void logout()}
          >
            {$t("dino.logout")}
          </button>
        {:else}
          <button
            class="cursor-pointer rounded px-3 py-1 text-sm font-medium disabled:opacity-50"
            style="background: var(--color-accent); color: var(--color-bg)"
            disabled={loginBusy}
            onclick={() => void tokenLogin()}
          >
            {$t("dino.login")}
          </button>
          {#if loginBusy}
            <span class="text-sm" style="color: var(--color-muted)">
              {$t("dino.login_wait")}
            </span>
            <button
              class="cursor-pointer rounded border px-2 py-0.5 text-xs"
              style="border-color: var(--color-border)"
              onclick={() => void cancelLogin()}
            >
              {$t("dino.cancel_login")}
            </button>
          {/if}
        {/if}
      </div>

      <!-- Manual token paste (escape hatch) -->
      <details class="mt-3">
        <summary class="cursor-pointer text-xs" style="color: var(--color-muted)">
          {$t("dino.token_paste")}
        </summary>
        <p class="mb-2 mt-1 text-xs leading-relaxed" style="color: var(--color-muted)">
          {$t("dino.token_paste_hint")}
        </p>
        <textarea
          class="mb-2 w-full rounded border px-2 py-1 font-mono text-xs"
          style="border-color: var(--color-border); background: var(--color-bg); color: var(--color-text)"
          rows="2"
          placeholder="theisle-overlay://?sid=…&token=…"
          bind:value={tokenInput}
        ></textarea>
        {#if tokenError}
          <p class="mb-2 text-xs" style="color: #ff8a80">{$t("dino.token_bad")}</p>
        {/if}
        <button
          class="cursor-pointer rounded border px-3 py-1 text-sm disabled:opacity-50"
          style="border-color: var(--color-border)"
          disabled={tokenBusy || !tokenInput.trim()}
          onclick={() => void saveToken()}
        >
          {tokenBusy ? $t("dino.token_checking") : $t("dino.token_save")}
        </button>
      </details>

      <!-- Legacy fallback: per-server URL + cookie -->
      <details class="mt-3 border-t pt-3" style="border-color: var(--color-border)">
        <summary class="cursor-pointer text-xs font-semibold" style="color: var(--color-muted)">
          {$t("dino.legacy_section")}
        </summary>
        <p class="mb-2 mt-1 text-xs" style="color: var(--color-muted)">
          {$t("dino.legacy_hint")}
        </p>
        <div class="mb-1 text-sm font-semibold">{$t("dino.server")}</div>
        <p class="mb-2 text-xs" style="color: var(--color-muted)">
          {$t("dino.supported_servers")}
        </p>
        <input
          class="mb-3 w-full rounded border px-2 py-1 font-mono text-sm"
          style="border-color: var(--color-border); background: var(--color-bg); color: var(--color-text)"
          bind:value={domainInput}
          placeholder="https://…islepilot.eu"
        />
        <div class="flex items-center gap-3">
          {#if loggedIn && authMode === "legacy" && domainInput === settings.islepilot.domain && !authExpired}
            <span class="text-sm" style="color: #72d653">✓ {$t("dino.logged_in")}</span>
            <button
              class="cursor-pointer rounded border px-3 py-1 text-sm"
              style="border-color: var(--color-border)"
              onclick={() => void logout()}
            >
              {$t("dino.logout")}
            </button>
          {:else}
            <button
              class="cursor-pointer rounded px-3 py-1 text-sm font-medium disabled:opacity-50"
              style="background: var(--color-accent); color: var(--color-bg)"
              disabled={loginBusy || !domainInput.startsWith("https://")}
              onclick={() => void login()}
            >
              {$t("dino.login")}
            </button>
            {#if loginBusy}
              <span class="text-sm" style="color: var(--color-muted)">
                {$t("dino.login_wait")}
              </span>
              <button
                class="cursor-pointer rounded border px-2 py-0.5 text-xs"
                style="border-color: var(--color-border)"
                onclick={() => void cancelLogin()}
              >
                {$t("dino.cancel_login")}
              </button>
            {/if}
          {/if}
        </div>

        <!-- Cookie paste: the reliable legacy path -->
        <div class="mt-3 border-t pt-3" style="border-color: var(--color-border)">
          <div class="text-sm font-semibold" style="color: var(--color-accent)">
            {$t("dino.manual_cookie")}
          </div>
          <p class="mb-2 mt-1 text-xs leading-relaxed" style="color: var(--color-muted)">
            {$t("dino.manual_cookie_hint")}
          </p>
          <textarea
            class="mb-2 w-full rounded border px-2 py-1 font-mono text-xs"
            style="border-color: var(--color-border); background: var(--color-bg); color: var(--color-text)"
            rows="3"
            placeholder="islepilot_player=eyJhbGciOi…  (hoặc chỉ dán phần Value)"
            bind:value={cookieInput}
          ></textarea>
          {#if cookieError}
            <p class="mb-2 text-xs" style="color: #ff8a80">{$t("dino.manual_cookie_bad")}</p>
          {/if}
          <button
            class="cursor-pointer rounded border px-3 py-1 text-sm disabled:opacity-50"
            style="border-color: var(--color-border)"
            disabled={cookieBusy || !cookieInput.trim() || !domainInput.startsWith("https://")}
            onclick={() => void saveCookie()}
          >
            {cookieBusy ? $t("dino.manual_cookie_checking") : $t("dino.manual_cookie_save")}
          </button>
        </div>
      </details>
    </section>
    {/if}

    <!-- Options -->
    <section
      class="space-y-2 rounded border p-3"
      style="border-color: var(--color-border); background: var(--color-panel)"
    >
      <label class="flex cursor-pointer items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={settings.islepilot.enabled}
          onchange={(e) =>
            void patch({ islepilot: { enabled: e.currentTarget.checked } }, true)}
        />
        {$t("dino.enabled")}
      </label>
      <label class="flex cursor-pointer items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={settings.islepilot.show_overlay_panel}
          onchange={(e) =>
            void patch({ islepilot: { show_overlay_panel: e.currentTarget.checked } })}
        />
        {$t("dino.overlay_panel")}
      </label>
      <label class="flex cursor-pointer items-center gap-2 text-sm">
        <input
          type="checkbox"
          checked={settings.islepilot.show_quests_panel}
          onchange={(e) =>
            void patch({ islepilot: { show_quests_panel: e.currentTarget.checked } })}
        />
        {$t("dino.quests_panel")}
      </label>
      <!-- Live-map position: driven by the server's capability. No live map
           -> forced off and not clickable; live map -> on by default, and a
           manual flip marks map_pref_user_set so auto-on never overrides. -->
      <label
        class="flex items-center gap-2 text-sm {liveMap === false
          ? 'opacity-50'
          : 'cursor-pointer'}"
      >
        <input
          type="checkbox"
          checked={settings.islepilot.use_map_position}
          disabled={liveMap === false}
          onchange={(e) =>
            void patch(
              {
                islepilot: {
                  use_map_position: e.currentTarget.checked,
                  map_pref_user_set: true,
                },
              },
              true,
            )}
        />
        {$t("dino.use_map_position")}
      </label>
      {#if settings.islepilot.enabled && loggedIn}
        {#if liveMap === true}
          <p class="pl-6 text-xs" style="color: #72d653">✓ {$t("dino.live_map_yes")}</p>
        {:else if liveMap === false}
          <p class="pl-6 text-xs" style="color: var(--color-muted)">
            ✗ {$t("dino.map_disabled")}
          </p>
        {:else}
          <p class="pl-6 text-xs" style="color: var(--color-muted)">
            {$t("dino.live_map_checking")}
          </p>
        {/if}
      {/if}
      <label class="block text-sm">
        <div class="mb-0.5 flex justify-between">
          <span>{$t("dino.interval")}</span>
          <span class="font-mono" style="color: var(--color-muted)">
            {settings.islepilot.poll_interval_s}s
          </span>
        </div>
        <input
          type="range"
          class="w-full accent-[#e8a33d]"
          min="5"
          max="60"
          step="5"
          value={settings.islepilot.poll_interval_s}
          oninput={(e) =>
            void patch(
              { islepilot: { poll_interval_s: Number(e.currentTarget.value) } },
              true,
            )}
        />
      </label>
    </section>

    <!-- Stats -->
    <section
      class="rounded border p-4"
      style="border-color: var(--color-border); background: var(--color-panel)"
    >
      {#if player}
        <div class="mb-3 flex flex-wrap items-center gap-2">
          <span class="text-lg font-semibold">{player.dinoName ?? "?"}</span>
          {#if player.female !== null && player.female !== undefined}
            <span class="text-xs" style="color: var(--color-muted)">
              {player.female ? `♀ ${$t("dino.sex_female")}` : `♂ ${$t("dino.sex_male")}`}
            </span>
          {/if}
          {#if player.online !== null}
            <span
              class="rounded-full px-2 py-0.5 text-xs font-medium"
              style={player.online
                ? "background: #1e3a2f; color: #72d653"
                : "background: #3a2222; color: #ff8a80"}
            >
              {player.online ? $t("dino.online") : $t("dino.offline")}
            </span>
          {/if}
          {#if player.server}
            <span class="text-xs" style="color: var(--color-muted)">
              {$t("dino.server_playing")}: <span style="color: var(--color-text)">{player.server}</span>
            </span>
          {/if}
          {#if update}
            <span class="ml-auto text-xs" style="color: var(--color-muted)">
              {$t("dino.updated", { time: timeStr(update.fetchedAtMs) })}
            </span>
          {/if}
        </div>

        <!-- Growth -->
        <div class="mb-2">
          <div class="mb-0.5 flex justify-between text-sm">
            <span>{$t("dino.growth")}</span>
            <span class="font-mono">{player.growth ?? "—"}</span>
          </div>
          <div class="h-2 rounded" style="background: var(--color-bg)">
            <div
              class="h-2 rounded"
              style="width: {player.growthPct ?? 0}%; background: var(--color-accent)"
            ></div>
          </div>
        </div>

        {#each [["dino.health", player.health, hpColor(pct(player.health))], ["dino.hunger", player.hunger, "#e8a33d"], ["dino.thirst", player.thirst, "#4aa8d8"], ...(player.stamina ? [["dino.stamina", player.stamina, "#a78bfa"]] : [])] as [labelKey, bar, color] (labelKey)}
          <div class="mb-2">
            <div class="mb-0.5 flex justify-between text-sm">
              <span>{$t(labelKey as never)}</span>
              <span class="font-mono">{(bar as DinoStatBar | null)?.raw ?? "—"}</span>
            </div>
            <div class="h-2 rounded" style="background: var(--color-bg)">
              <div
                class="h-2 rounded"
                style="width: {pct(bar as DinoStatBar | null) ?? 0}%; background: {color}"
              ></div>
            </div>
          </div>
        {/each}

        <!-- Nutrition (token mode only) -->
        {#if nutrition}
          <div class="mb-2 mt-3">
            <div class="mb-1 text-sm font-semibold" style="color: var(--color-accent)">
              {$t("dino.nutrition")}
            </div>
            <div class="flex gap-4 font-mono text-sm">
              <span title={$t("dino.nutrition_carb")}>
                🌾 {$t("dino.nutrition_carb")}: {nutrition.carb.toFixed(1)}
              </span>
              <span title={$t("dino.nutrition_protein")}>
                🍖 {$t("dino.nutrition_protein")}: {nutrition.protein.toFixed(1)}
              </span>
              <span title={$t("dino.nutrition_lipid")}>
                🧈 {$t("dino.nutrition_lipid")}: {nutrition.lipid.toFixed(1)}
              </span>
            </div>
          </div>
        {/if}

        <!-- Prime progress -->
        {#if player.primeQuests.length > 0}
          <h3 class="mb-1 mt-4 text-sm font-semibold" style="color: var(--color-accent)">
            {$t("dino.prime")}
            <span class="font-normal" style="color: var(--color-muted)">
              ({player.primeQuests.filter((q) => q.completed).length}/{player.primeQuests.length})
            </span>
          </h3>
          <ul class="space-y-1">
            {#each player.primeQuests as quest (quest.text)}
              <li class="flex items-start gap-2 text-sm">
                <span style="color: {quest.completed ? '#72d653' : 'var(--color-muted)'}">
                  {quest.completed ? "✓" : "○"}
                </span>
                <!-- Vietnamese when available; the English original stays a
                     hover away so in-game terms remain checkable. -->
                <span
                  style={quest.completed ? "color: #72d653" : ""}
                  title={$locale === "vi" && quest.textVi ? quest.text : undefined}
                >
                  {$locale === "vi" ? (quest.textVi ?? quest.text) : quest.text}
                </span>
              </li>
            {/each}
          </ul>
        {/if}

        {#if update?.layoutChanged}
          <p class="mt-3 text-xs" style="color: #ffd591">{$t("dino.layout_changed")}</p>
        {/if}
      {:else if update?.error}
        <p class="text-sm" style="color: #ff8a80">
          {$t("dino.fetch_error")} <span class="font-mono text-xs">{update.error}</span>
        </p>
      {:else}
        <p class="text-sm" style="color: var(--color-muted)">{$t("dino.no_data")}</p>
      {/if}
    </section>

  </div>
{/if}
