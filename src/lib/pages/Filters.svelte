<script lang="ts">
  import { config, busy, setBusy, pushLog } from "../stores";
  import { api } from "../api";
  import { onMount } from "svelte";

  let gameFilter = $state(false);
  let ipsetMode = $state("none");

  onMount(async () => {
    try {
      const st = await api.filterState();
      gameFilter = st.game_filter;
      ipsetMode = st.ipset_mode;
    } catch (e) {
      pushLog({ ts: new Date().toISOString(), level: "error", message: String(e) });
    }
  });

  async function toggleGame() {
    gameFilter = !gameFilter;
    setBusy("game", true);
    try {
      await api.setGameFilter(gameFilter);
      pushLog({
        ts: new Date().toISOString(),
        level: "info",
        message: `Game Filter → ${gameFilter ? "on" : "off"}`,
      });
    } catch (e) {
      gameFilter = !gameFilter;
      pushLog({ ts: new Date().toISOString(), level: "error", message: String(e) });
    } finally {
      setBusy("game", false);
    }
  }

  async function setIpset(mode: string) {
    ipsetMode = mode;
    setBusy("ipset", true);
    try {
      await api.setIpsetMode(mode);
      pushLog({
        ts: new Date().toISOString(),
        level: "info",
        message: `IPSet mode → ${mode}`,
      });
    } catch (e) {
      pushLog({ ts: new Date().toISOString(), level: "error", message: String(e) });
    } finally {
      setBusy("ipset", false);
    }
  }

  const ipsetOptions = [
    { id: "none", label: "Выключен", desc: "IP-список не используется." },
    { id: "loaded", label: "По списку (ipset-all.txt)", desc: "Фильтровать только IP из списка." },
    { id: "any", label: "Все IP", desc: "Не рекомендуется для постоянного использования." },
  ];
</script>

<div class="p-6 space-y-6 max-w-3xl">
  <div>
    <h1 class="text-xl font-semibold text-ink-100">Фильтры</h1>
    <p class="text-sm text-ink-300 mt-1">
      Game Filter и IPSet Filter — оверлеи поверх активной стратегии. После изменения требуется
      перезапуск сервиса.
    </p>
  </div>

  <div class="card">
    <div class="flex items-start justify-between gap-4">
      <div>
        <h2 class="font-semibold text-ink-100">Game Filter</h2>
        <p class="text-sm text-ink-300 mt-1">
          Включает обход для игр и сервисов по UDP/TCP на портах выше 1023. Первый шаг при
          проблемах с подключением к играм.
        </p>
      </div>
      <button
        class="switch {gameFilter ? 'bg-brand-600' : 'bg-ink-600'}"
        onclick={toggleGame}
        aria-label="Game Filter"
        disabled={$busy.game}
      >
        <span class="switch-knob {gameFilter ? 'translate-x-6' : 'translate-x-1'}"></span>
      </button>
    </div>
    <div class="mt-3 text-xs">
      {#if gameFilter}
        <span class="pill-ok">включен</span>
      {:else}
        <span class="pill-info">выключен</span>
      {/if}
    </div>
  </div>

  <div class="card">
    <h2 class="font-semibold text-ink-100">IPSet Filter</h2>
    <p class="text-sm text-ink-300 mt-1">
      Управляет тем, какие IP-адреса попадают под фильтр из <code class="text-ink-200">ipset-all.txt</code>.
    </p>
    <div class="mt-4 space-y-2">
      {#each ipsetOptions as opt (opt.id)}
        <button
          class="w-full flex items-start gap-3 p-3 rounded-lg text-left border transition-colors
            {ipsetMode === opt.id
            ? 'bg-brand-600/10 border-brand-600/40'
            : 'bg-ink-900 border-ink-700 hover:border-ink-600'}"
          onclick={() => setIpset(opt.id)}
          disabled={$busy.ipset}
        >
          <span
            class="mt-1 h-3 w-3 rounded-full border-2 flex-shrink-0
              {ipsetMode === opt.id ? 'border-brand-400 bg-brand-400' : 'border-ink-500'}"
          ></span>
          <div>
            <div class="text-sm font-medium text-ink-100">{opt.label}</div>
            <div class="text-xs text-ink-400">{opt.desc}</div>
          </div>
        </button>
      {/each}
    </div>
  </div>

  <div class="card border-sky-500/20 bg-sky-500/5">
    <div class="flex items-start gap-3">
      <span class="text-sky-300 text-lg">↻</span>
      <div>
        <div class="font-medium text-sky-200">Перезапуск обязателен</div>
        <p class="text-sm text-ink-300 mt-1">
          Изменения вступают в силу только после перезапуска сервиса — переключитесь на вкладку
          «Статус» и нажмите «Стоп» → «Старт».
        </p>
      </div>
    </div>
  </div>
</div>
