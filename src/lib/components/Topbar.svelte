<script lang="ts">
  import { status, serviceRunning, healthOk } from "../stores";
  import { api } from "../api";
  import { setBusy, busy } from "../stores";

  async function toggle() {
    setBusy("toggle", true);
    try {
      const s = await api.serviceSnapshot();
      if (s.state === "running") {
        await api.serviceStop();
      } else {
        await api.serviceStart();
      }
    } catch (e) {
      console.error(e);
    } finally {
      setBusy("toggle", false);
    }
  }

  let stateLabel = $derived.by(() => {
    const s = $status;
    if (!s) return "—";
    if (s.state === "running") return "Сервис работает";
    if (s.state === "stopped") return "Сервис остановлен";
    if (s.adhoc_running) return "Фоновый процесс";
    return s.state;
  });

  let dotColor = $derived.by(() => {
    const s = $status;
    if (!s) return "bg-ink-400";
    if (s.state === "running") return "bg-emerald-400";
    if (s.adhoc_running) return "bg-emerald-400";
    if (s.state === "stopped") return "bg-red-400";
    return "bg-amber-400";
  });
</script>

<header
  class="flex h-14 items-center gap-4 px-6 bg-ink-800 border-b border-ink-700"
>
  <div class="flex items-center gap-2.5">
    <span class="relative inline-flex h-2.5 w-2.5">
      <span
        class="absolute inline-flex h-full w-full rounded-full opacity-60 animate-ping {$serviceRunning
          ? 'bg-emerald-400'
          : 'bg-red-400'}"
      ></span>
      <span class="relative inline-flex rounded-full h-2.5 w-2.5 {dotColor}"></span>
    </span>
    <div class="text-sm">
      <div class="font-medium text-ink-100">{stateLabel}</div>
      <div class="text-[11px] text-ink-400">
        {$healthOk
          ? "Обход работает"
          : $status?.state === "running"
            ? "Сервис вверх, но здоровья нет"
            : "—"}
      </div>
    </div>
  </div>

  <div class="flex-1"></div>

  {#if $status}
    <div class="text-[11px] text-ink-400">
      {#if $status.pid}
        PID {$status.pid}
      {/if}
      {#if $status.health_latency_ms != null}
        · {$status.health_latency_ms} ms
      {/if}
    </div>
  {/if}

  <button
    class="btn-primary"
    onclick={toggle}
    disabled={$busy.toggle}
  >
    {$serviceRunning ? "Стоп" : "Старт"}
  </button>
</header>
