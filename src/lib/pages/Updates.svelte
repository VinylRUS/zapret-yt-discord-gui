<script lang="ts">
  import { config, presets, busy, setBusy, pushLog } from "../stores";
  import { api, type UpdateCheckResult } from "../api";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { onMount } from "svelte";

  let upstream = $state<UpdateCheckResult | null>(null);
  let pendingUpdate = $state<Update | null>(null);
  let selfUpdateAvailable = $state(false);
  let selfUpdateVersion = $state<string | null>(null);
  let installing = $state(false);
  let installProgress = $state<number | null>(null);
  let installError = $state<string | null>(null);

  onMount(async () => {
    try {
      upstream = await api.checkUpstreamUpdate();
    } catch (e) {
      pushLog({ ts: new Date().toISOString(), level: "error", message: String(e) });
    }
    try {
      const u = await check();
      if (u.available) {
        selfUpdateAvailable = true;
        selfUpdateVersion = u.version;
        pendingUpdate = u;
      }
    } catch (e) {
      // updater not configured (no pubkey / no endpoint) — record and ignore
      pushLog({
        ts: new Date().toISOString(),
        level: "warn",
        message: `self-update unavailable: ${e}`,
      });
    }
  });

  async function installSelfUpdate() {
    if (!pendingUpdate || installing) return;
    installing = true;
    installProgress = 0;
    installError = null;
    pushLog({
      ts: new Date().toISOString(),
      level: "info",
      message: `installing GUI update ${pendingUpdate.version}…`,
    });
    try {
      let total = 0;
      let downloaded = 0;
      await pendingUpdate.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            total = event.data.contentLength ?? 0;
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            installProgress = total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : null;
            break;
          case "Finished":
            installProgress = 100;
            break;
        }
      });
      pushLog({
        ts: new Date().toISOString(),
        level: "info",
        message: `GUI update downloaded, relaunching…`,
      });
      await relaunch();
    } catch (e) {
      installError = String(e);
      pushLog({
        ts: new Date().toISOString(),
        level: "error",
        message: `GUI update failed: ${e}`,
      });
    } finally {
      installing = false;
    }
  }

  async function run(fn: () => Promise<unknown>, key: string, label: string) {
    setBusy(key, true);
    pushLog({ ts: new Date().toISOString(), level: "info", message: `${label}…` });
    try {
      const r = await fn();
      pushLog({ ts: new Date().toISOString(), level: "info", message: `${label} ✓ ${r ?? ""}` });
      upstream = await api.checkUpstreamUpdate();
    } catch (e) {
      pushLog({ ts: new Date().toISOString(), level: "error", message: `${label}: ${e}` });
    } finally {
      setBusy(key, false);
    }
  }

  async function refreshStrategies() {
    setBusy("strat", true);
    pushLog({ ts: new Date().toISOString(), level: "info", message: "Стратегии flowseal…" });
    try {
      const n = await api.refreshStrategies();
      pushLog({
        ts: new Date().toISOString(),
        level: "info",
        message: `Стратегии flowseal ✓ загружено ${n} пресетов`,
      });
      // reload the preset store so the Strategies page reflects the new list
      presets.set(await api.listPresets());
    } catch (e) {
      pushLog({ ts: new Date().toISOString(), level: "error", message: `Стратегии: ${e}` });
    } finally {
      setBusy("strat", false);
    }
  }
</script>

<div class="p-6 space-y-6 max-w-3xl">
  <div>
    <h1 class="text-xl font-semibold text-ink-100">Обновления</h1>
    <p class="text-sm text-ink-300 mt-1">
      Автоматическая проверка апдейтов с GitHub и обновление компонент zapret.
    </p>
  </div>

  {#if selfUpdateAvailable}
    <div class="card border-brand-600/30 bg-brand-600/5">
      <div class="flex items-start gap-3">
        <span class="text-brand-300 text-lg">↑</span>
        <div class="flex-1">
          <div class="font-medium text-brand-200">
            Доступно обновление GUI: {selfUpdateVersion}
          </div>
          <p class="text-sm text-ink-300 mt-1">
            Обновление будет установлено через плагин updater и потребует перезапуска.
          </p>
          {#if installProgress !== null && installing}
            <div class="mt-3">
              <div class="h-1.5 w-full bg-ink-700 rounded-full overflow-hidden">
                <div class="h-full bg-brand-500 transition-all" style="width:{installProgress}%"></div>
              </div>
              <div class="text-xs text-ink-400 mt-1">Загрузка… {installProgress}%</div>
            </div>
          {/if}
          {#if installError}
            <div class="text-sm text-red-300 mt-2">Ошибка: {installError}</div>
          {/if}
          <div class="flex gap-2 mt-3">
            <button
              class="btn-primary"
              onclick={installSelfUpdate}
              disabled={installing}
            >
              {installing ? "Установка…" : "Установить обновление"}
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <div class="card">
    <div class="flex items-start justify-between gap-4">
      <div>
        <h2 class="font-semibold text-ink-100">zapret binaries</h2>
        <p class="text-sm text-ink-300 mt-1">
          Текущая версия: <strong>{$config?.zapret_bin_version || "не установлена"}</strong>
        </p>
        {#if upstream?.has_update}
          <p class="text-sm text-amber-300 mt-1">
            Доступно: {upstream.latest?.tag_name} ({new Date(
              upstream.latest?.published_at ?? Date.now(),
            ).toLocaleDateString()})
          </p>
        {:else if upstream}
          <p class="text-sm text-emerald-300 mt-1">У вас последняя версия.</p>
        {/if}
      </div>
      <div class="flex flex-col gap-2">
        <button
          class="btn-secondary"
          onclick={() => run(() => api.checkUpstreamUpdate(), "check", "Проверка апдейта")}
          disabled={$busy.check}
        >
          Проверить
        </button>
        <button
          class="btn-primary"
          onclick={() => run(() => api.updateZapretBinaries(), "bin", "Обновление bin")}
          disabled={$busy.bin}
        >
          Update zapret binaries
        </button>
        <button
          class="btn-secondary"
          onclick={refreshStrategies}
          disabled={$busy.strat}
          title="Скачать general*.bat из Flowseal/zapret-discord-youtube и перестроить список стратегий"
        >
          Обновить стратегии
        </button>
      </div>
    </div>
  </div>

  <div class="card">
    <h2 class="font-semibold text-ink-100">Списки и hosts</h2>
    <p class="text-sm text-ink-300 mt-1 mb-4">
      Обновление IPSet-списка и hosts-файла (Telegram web / Discord voice).
    </p>
    <div class="flex flex-wrap gap-2">
      <button
        class="btn-secondary"
        onclick={() => run(() => api.updateIpsetList(), "ipset", "IPSet list")}
        disabled={$busy.ipset}
      >
        Update IPSet list
      </button>
      <button
        class="btn-secondary"
        onclick={() => run(() => api.updateHostsFile(), "hosts", "hosts file")}
        disabled={$busy.hosts}
      >
        Update hosts file
      </button>
    </div>
  </div>

  {#if upstream?.latest}
    <div class="card">
      <h2 class="font-semibold text-ink-100">Release notes</h2>
      <div class="text-xs text-ink-400 mt-1 mb-2">
        {upstream.latest.tag_name} ·
        <a
          href={upstream.latest.html_url}
          target="_blank"
          rel="noopener"
          class="text-brand-300 hover:underline"
        >открыть на GitHub</a>
      </div>
      <pre class="text-xs text-ink-300 whitespace-pre-wrap max-h-64 overflow-y-auto bg-ink-900 rounded p-3">{upstream.latest.body || "— без описания —"}</pre>
    </div>
  {/if}
</div>
