<script lang="ts">
  import { config, logs, pushLog } from "../stores";
  import { api, type AppConfig } from "../api";
  import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
  import { onMount } from "svelte";

  let local = $state<AppConfig | null>(null);
  let autostartOn = $state(false);
  let saving = $state(false);

  onMount(async () => {
    local = structuredClone($config);
    if (!local) local = await api.getConfig();
    try {
      autostartOn = await isEnabled();
    } catch {
      autostartOn = false;
    }
  });

  async function save() {
    if (!local) return;
    saving = true;
    try {
      await api.saveConfig(local);
      pushLog({ ts: new Date().toISOString(), level: "info", message: "config saved" });
    } catch (e) {
      pushLog({ ts: new Date().toISOString(), level: "error", message: String(e) });
    } finally {
      saving = false;
    }
  }

  async function toggleAutostart() {
    try {
      if (autostartOn) {
        await disable();
        autostartOn = false;
      } else {
        await enable();
        autostartOn = true;
      }
    } catch (e) {
      pushLog({ ts: new Date().toISOString(), level: "error", message: String(e) });
    }
  }

  let logLines = $derived($logs.slice(-50).reverse());
</script>

<div class="p-6 space-y-6 max-w-3xl">
  <div>
    <h1 class="text-xl font-semibold text-ink-100">Настройки</h1>
    <p class="text-sm text-ink-300 mt-1">Параметры приложения и журнала.</p>
  </div>

  {#if local}
    <div class="card space-y-4">
      <h2 class="font-semibold text-ink-100">Основные</h2>

      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm font-medium text-ink-100">Минимизировать в трей</div>
          <div class="text-xs text-ink-400">Вместо закрытия окна.</div>
        </div>
        <button
          class="switch {local.minimize_to_tray ? 'bg-brand-600' : 'bg-ink-600'}"
          aria-label="Минимизировать в трей"
          aria-pressed={local.minimize_to_tray}
          onclick={() => (local!.minimize_to_tray = !local!.minimize_to_tray)}
        >
          <span
            class="switch-knob {local.minimize_to_tray
              ? 'translate-x-6'
              : 'translate-x-1'}"
          ></span>
        </button>
      </div>

      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm font-medium text-ink-100">Автопроверка апдейтов</div>
          <div class="text-xs text-ink-400">При запуске приложения.</div>
        </div>
        <button
          class="switch {local.auto_update_check ? 'bg-brand-600' : 'bg-ink-600'}"
          aria-label="Автопроверка апдейтов"
          aria-pressed={local.auto_update_check}
          onclick={() => (local!.auto_update_check = !local!.auto_update_check)}
        >
          <span
            class="switch-knob {local.auto_update_check
              ? 'translate-x-6'
              : 'translate-x-1'}"
          ></span>
        </button>
      </div>

      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm font-medium text-ink-100">Запускать с Windows</div>
          <div class="text-xs text-ink-400">Автозапуск приложения.</div>
        </div>
        <button
          class="switch {autostartOn ? 'bg-brand-600' : 'bg-ink-600'}"
          aria-label="Запускать с Windows"
          aria-pressed={autostartOn}
          onclick={toggleAutostart}
        >
          <span class="switch-knob {autostartOn ? 'translate-x-6' : 'translate-x-1'}"></span>
        </button>
      </div>

      <div>
        <label class="label" for="interval">Интервал проверки апдейтов (часы)</label>
        <input
          id="interval"
          type="number"
          min="1"
          max="168"
          class="field mt-1 w-32"
          bind:value={local.update_interval_hours}
        />
      </div>

      <div>
        <label class="label" for="theme">Тема</label>
        <select id="theme" class="field mt-1 w-48" bind:value={local.theme}>
          <option value="dark">Тёмная</option>
          <option value="light">Светлая</option>
        </select>
      </div>

      <div class="flex gap-2 pt-2">
        <button class="btn-primary" onclick={save} disabled={saving}>
          {saving ? "Сохранение…" : "Сохранить"}
        </button>
      </div>
    </div>

    <div class="card">
      <div class="flex items-center justify-between">
        <h2 class="font-semibold text-ink-100">Каталог приложения</h2>
        <button
          class="btn-secondary"
          onclick={async () => {
            const path = await api.getAppDir();
            await api.openPath(path);
          }}
        >
          Открыть папку
        </button>
      </div>
      <p class="text-xs text-ink-400 mt-2">
        Папка установки (рядом с exe): здесь хранятся winws.exe, списки,
        config.toml, presets.json. Логи — отдельно в %APPDATA%\zapret-gui\logs.
      </p>
    </div>

    <div class="card">
      <h2 class="font-semibold text-ink-100 mb-3">Журнал</h2>
      <div class="bg-ink-900 rounded-lg p-3 max-h-64 overflow-y-auto font-mono text-[11px]">
        {#if logLines.length === 0}
          <div class="text-ink-400">журнал пуст</div>
        {:else}
          {#each logLines as line (line.ts)}
            <div class="flex gap-2">
              <span class="text-ink-500">{new Date(line.ts).toLocaleTimeString()}</span>
              <span class={line.level === 'error' ? 'text-red-400' : line.level === 'warn' ? 'text-amber-400' : 'text-ink-300'}>[{line.level}]</span>
              <span class="text-ink-200">{line.message}</span>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  {/if}
</div>
