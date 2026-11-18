<script lang="ts">
  import { status, config, presets, busy, setBusy, pushLog } from "../stores";
  import { api, type ServiceSnapshot } from "../api";
  import { onMount } from "svelte";

  let snap = $state<ServiceSnapshot | null>(null);
  let binOk = $state(false);

  async function refresh() {
    snap = await api.serviceSnapshot();
    binOk = await api.binExists();
  }

  onMount(() => {
    refresh();
    const i = setInterval(refresh, 5000);
    return () => clearInterval(i);
  });

  async function run(fn: () => Promise<void>, key: string) {
    setBusy(key, true);
    try {
      await fn();
      await refresh();
    } catch (e) {
      pushLog({ ts: new Date().toISOString(), level: "error", message: String(e) });
    } finally {
      setBusy(key, false);
    }
  }

  let activePresetName = $derived.by(() => {
    const cfg = $config;
    const p = $presets;
    if (!cfg || !p) return "—";
    return p.presets.find((x) => x.id === cfg.active_strategy)?.name ?? cfg.active_strategy;
  });
</script>

<div class="p-6 space-y-6 max-w-4xl">
  <div>
    <h1 class="text-xl font-semibold text-ink-100">Статус</h1>
    <p class="text-sm text-ink-300 mt-1">
      Управление Windows-сервисом Zapret и быстрый запуск без установки.
    </p>
  </div>

  {#if !binOk}
    <div class="card border-amber-500/30 bg-amber-500/5">
      <div class="flex items-start gap-3">
        <span class="text-amber-300 text-lg">⚠</span>
        <div class="flex-1">
          <div class="font-medium text-amber-200">Бинарники zapret не установлены</div>
          <p class="text-sm text-ink-300 mt-1">
            Скачайте их с вкладки <strong>Обновления → Update zapret binaries</strong>,
            после чего вернитесь сюда и установите сервис.
          </p>
        </div>
      </div>
    </div>
  {/if}

  <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
    <div class="card">
      <div class="label">Состояние сервиса</div>
      <div class="text-2xl font-semibold mt-2">
        {#if !snap?.installed}
          <span class="text-ink-300">Не установлен</span>
        {:else if snap.state === "running"}
          <span class="text-emerald-300">Работает</span>
        {:else}
          <span class="text-red-300">Остановлен</span>
        {/if}
      </div>
      <div class="text-xs text-ink-400 mt-1">
        {#if snap?.installed && snap.pid}
          PID {snap.pid}
        {/if}
      </div>
    </div>

    <div class="card">
      <div class="label">Активная стратегия</div>
      <div class="text-lg font-medium mt-2">{activePresetName}</div>
      <div class="text-xs text-ink-400 mt-1">
        Изменить — вкладка «Стратегии»
      </div>
    </div>

    <div class="card">
      <div class="label">Здоровье обхода</div>
      <div class="text-2xl font-semibold mt-2">
        {#if $status?.health_ok}
          <span class="text-emerald-300">OK</span>
        {:else}
          <span class="text-red-300">Нет</span>
        {/if}
      </div>
      <div class="text-xs text-ink-400 mt-1">
        {#if $status?.health_latency_ms != null}
          задержка {$status.health_latency_ms} ms
        {/if}
      </div>
    </div>
  </div>

  <div class="card">
    <h2 class="font-semibold text-ink-100">Управление сервисом</h2>
    <p class="text-sm text-ink-300 mt-1 mb-4">
      Сервис запускается автоматически при старте Windows. Установка требует прав администратора.
    </p>
    <div class="flex flex-wrap gap-2">
      <button
        class="btn-primary"
        onclick={() => run(api.serviceInstall, "install")}
        disabled={$busy.install || !binOk}
      >
        Установить сервис
      </button>
      <button
        class="btn-secondary"
        onclick={() => run(api.serviceStart, "start")}
        disabled={$busy.start || !snap?.installed}
      >
        Запустить
      </button>
      <button
        class="btn-secondary"
        onclick={() => run(api.serviceStop, "stop")}
        disabled={$busy.stop || snap?.state !== "running"}
      >
        Остановить
      </button>
      <button
        class="btn-danger"
        onclick={() => run(api.serviceRemove, "remove")}
        disabled={$busy.remove || !snap?.installed}
      >
        Удалить сервис
      </button>
    </div>
  </div>

  <div class="card">
    <h2 class="font-semibold text-ink-100">Быстрый запуск (без сервиса)</h2>
    <p class="text-sm text-ink-300 mt-1 mb-4">
      Запускает winws.exe как обычный процесс с текущей стратегией. Удобно для теста.
    </p>
    <div class="flex flex-wrap gap-2">
      {#if $status?.adhoc_running}
        <button class="btn-danger" onclick={() => run(api.processStop, "pstop")}>
          Остановить процесс (PID {$status.adhoc_pid})
        </button>
      {:else}
        <button
          class="btn-primary"
          onclick={() => run(() => api.processStart(null), "pstart")}
          disabled={!binOk}
        >
          Запустить процесс
        </button>
      {/if}
    </div>
  </div>
</div>
