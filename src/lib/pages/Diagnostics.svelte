<script lang="ts">
  import { api, type DiagnosticReport } from "../api";
  import { onMount } from "svelte";

  let report = $state<DiagnosticReport | null>(null);
  let loading = $state(false);

  async function run() {
    loading = true;
    try {
      report = await api.runDiagnostics();
    } finally {
      loading = false;
    }
  }

  onMount(run);

  const outcomePill = {
    ok: "pill-ok",
    warn: "pill-warn",
    fail: "pill-fail",
    info: "pill-info",
  } as const;

  const outcomeLabel = {
    ok: "OK",
    warn: "Внимание",
    fail: "Ошибка",
    info: "Инфо",
  } as const;
</script>

<div class="p-6 space-y-6 max-w-4xl">
  <div class="flex items-end justify-between gap-4">
    <div>
      <h1 class="text-xl font-semibold text-ink-100">Диагностика</h1>
      <p class="text-sm text-ink-300 mt-1">
        Проверка типичных причин неработоспособности zapret.
      </p>
    </div>
    <button class="btn-primary" onclick={run} disabled={loading}>
      {loading ? "Проверяем…" : "Запустить проверку"}
    </button>
  </div>

  {#if report}
    <div class="grid grid-cols-3 gap-4">
      <div class="card text-center">
        <div class="label">OK</div>
        <div class="text-2xl font-semibold text-emerald-300 mt-1">{report.passed}</div>
      </div>
      <div class="card text-center">
        <div class="label">Внимание</div>
        <div class="text-2xl font-semibold text-amber-300 mt-1">{report.warned}</div>
      </div>
      <div class="card text-center">
        <div class="label">Ошибки</div>
        <div class="text-2xl font-semibold text-red-300 mt-1">{report.failed}</div>
      </div>
    </div>

    <div class="card">
      <h2 class="font-semibold text-ink-100 mb-3">Проверки</h2>
      <div class="space-y-2">
        {#each report.items as item (item.id)}
          <div class="flex items-start gap-3 p-3 rounded-lg bg-ink-900 border border-ink-700">
            <div class="flex-shrink-0 pt-0.5">
              <span class={outcomePill[item.outcome]}>{outcomeLabel[item.outcome]}</span>
            </div>
            <div class="flex-1 min-w-0">
              <div class="text-sm font-medium text-ink-100">{item.title}</div>
              <div class="text-xs text-ink-300 mt-0.5">{item.detail}</div>
              {#if item.hint}
                <div class="text-xs text-sky-300 mt-1">→ {item.hint}</div>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </div>

    <div class="card">
      <h2 class="font-semibold text-ink-100">Действия</h2>
      <div class="flex flex-wrap gap-2 mt-3">
        <button class="btn-secondary" onclick={() => api.clearDiscordCache()}>
          Очистить кэш Discord
        </button>
      </div>
    </div>
  {:else if loading}
    <div class="card text-center text-ink-400">Идёт диагностика…</div>
  {/if}
</div>
