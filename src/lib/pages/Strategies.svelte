<script lang="ts">
  import { presets, config, busy, setBusy, pushLog } from "../stores";
  import { api, type TestReport } from "../api";
  import { onMount } from "svelte";

  let selectedId = $state<string | null>(null);
  let report = $state<TestReport | null>(null);
  let running = $state(false);

  onMount(() => {
    if ($presets) {
      selectedId = $config?.active_strategy ?? $presets.presets[0]?.id ?? null;
    }
  });

  let selected = $derived.by(() => {
    if (!$presets || !selectedId) return null;
    return $presets.presets.find((p) => p.id === selectedId) ?? null;
  });

  let resolvedArgs = $state<string[]>([]);

  async function loadArgs(id: string) {
    try {
      resolvedArgs = await api.resolvePresetArgs(id);
    } catch (e) {
      resolvedArgs = [];
      pushLog({ ts: new Date().toISOString(), level: "error", message: String(e) });
    }
  }

  $effect(() => {
    if (selectedId) loadArgs(selectedId);
  });

  async function applyActive() {
    if (!selectedId) return;
    setBusy("applyStrategy", true);
    try {
      await api.setActiveStrategy(selectedId);
      pushLog({
        ts: new Date().toISOString(),
        level: "info",
        message: `активная стратегия → ${selectedId}`,
      });
    } catch (e) {
      pushLog({ ts: new Date().toISOString(), level: "error", message: String(e) });
    } finally {
      setBusy("applyStrategy", false);
    }
  }

  async function runTests() {
    running = true;
    setBusy("tests", true);
    try {
      report = await api.runStrategyTests(null, null);
    } catch (e) {
      pushLog({ ts: new Date().toISOString(), level: "error", message: String(e) });
    } finally {
      running = false;
      setBusy("tests", false);
    }
  }

  let grouped = $derived.by(() => {
    if (!$presets) return [] as { category: string; items: typeof $presets.presets }[];
    const map = new Map<string, typeof $presets.presets>();
    for (const p of $presets.presets) {
      const arr = map.get(p.category) ?? [];
      arr.push(p);
      map.set(p.category, arr);
    }
    return [...map.entries()].map(([category, items]) => ({ category, items }));
  });

  const categoryLabels: Record<string, string> = {
    general: "Основные",
    alt: "Альтернативные (ALT)",
    "fake-tls-auto": "Fake TLS Auto",
    "simple-fake": "Simple Fake",
    exp: "Экспериментальные",
  };
</script>

<div class="p-6 space-y-6 max-w-5xl">
  <div class="flex items-end justify-between gap-4">
    <div>
      <h1 class="text-xl font-semibold text-ink-100">Стратегии</h1>
      <p class="text-sm text-ink-300 mt-1">
        Выбор пресета обхода и автоматический подбор рабочей стратегии.
      </p>
    </div>
    <button class="btn-primary" onclick={runTests} disabled={running}>
      {running ? "Тестируем…" : "Подобрать стратегию"}
    </button>
  </div>

  <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
    <div class="lg:col-span-2 space-y-4">
      {#each grouped as group (group.category)}
        <div class="card">
          <h2 class="font-semibold text-ink-100 mb-3">
            {categoryLabels[group.category] ?? group.category}
          </h2>
          <div class="space-y-2">
            {#each group.items as p (p.id)}
              <button
                class="w-full flex items-start gap-3 p-3 rounded-lg text-left transition-colors border
                  {selectedId === p.id
                  ? 'bg-brand-600/10 border-brand-600/40'
                  : 'bg-ink-900 border-ink-700 hover:border-ink-600'}"
                onclick={() => (selectedId = p.id)}
              >
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-2">
                    <span class="font-medium text-sm text-ink-100">{p.name}</span>
                    {#if p.recommended}
                      <span class="pill-info">рекоменд.</span>
                    {/if}
                    {#if $config?.active_strategy === p.id}
                      <span class="pill-ok">активна</span>
                    {/if}
                  </div>
                  <div class="text-xs text-ink-400 mt-1">{p.description}</div>
                </div>
              </button>
            {/each}
          </div>
        </div>
      {/each}
    </div>

    <div class="space-y-4">
      <div class="card sticky top-0">
        <h2 class="font-semibold text-ink-100">Детали</h2>
        {#if selected}
          <div class="mt-3 space-y-2 text-sm">
            <div><span class="label">ID</span>
              <div class="font-mono text-xs text-ink-200 mt-1">{selected.id}</div>
            </div>
            <div><span class="label">Категория</span>
              <div class="text-ink-200 mt-1">{selected.category}</div>
            </div>
            <div>
              <span class="label">Аргументы winws.exe</span>
              <pre class="mt-1 text-[11px] font-mono text-ink-300 bg-ink-900 rounded p-2 overflow-x-auto max-h-48">{resolvedArgs.join("\n")}</pre>
            </div>
            <button
              class="btn-primary w-full mt-2"
              onclick={applyActive}
              disabled={$busy.applyStrategy || $config?.active_strategy === selected.id}
            >
              {$config?.active_strategy === selected.id ? "Уже активна" : "Сделать активной"}
            </button>
            <p class="text-[11px] text-ink-400">
              После смены стратегии перезапустите сервис на вкладке «Статус».
            </p>
          </div>
        {:else}
          <p class="text-sm text-ink-400 mt-3">Выберите пресет слева.</p>
        {/if}
      </div>
    </div>
  </div>

  {#if report}
    <div class="card">
      <h2 class="font-semibold text-ink-100">Результаты подбора</h2>
      <div class="text-xs text-ink-400 mt-1 mb-3">
        Цели: {report.targets.join(", ")} · начато {new Date(report.started_at).toLocaleString()}
      </div>
      <div class="overflow-x-auto">
        <table class="w-full text-sm">
          <thead>
            <tr class="text-left text-ink-400 text-xs uppercase">
              <th class="py-2 pr-4">Пресет</th>
              <th class="py-2 pr-4">Результат</th>
              <th class="py-2 pr-4">Задержка</th>
              <th class="py-2">Ошибка</th>
            </tr>
          </thead>
          <tbody>
            {#each report.results as r (r.preset_id)}
              <tr class="border-t border-ink-700">
                <td class="py-2 pr-4 text-ink-100">{r.preset_name}</td>
                <td class="py-2 pr-4">
                  {#if r.success}
                    <span class="pill-ok">✓ работает</span>
                  {:else}
                    <span class="pill-fail">✗ нет</span>
                  {/if}
                </td>
                <td class="py-2 pr-4 text-ink-300">
                  {r.latency_ms != null ? `${r.latency_ms} ms` : "—"}
                </td>
                <td class="py-2 text-ink-400 text-xs">{r.error ?? ""}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      {#if report.best_preset}
        <div class="mt-3 text-sm">
          Лучший пресет: <strong class="text-brand-300">{report.best_preset}</strong>
          <button
            class="btn-secondary ml-3"
            onclick={async () => {
              await api.setActiveStrategy(report!.best_preset!);
              selectedId = report!.best_preset!;
            }}
          >
            Сделать активной
          </button>
        </div>
      {/if}
    </div>
  {/if}
</div>
