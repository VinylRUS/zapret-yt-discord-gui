<script lang="ts">
  interface NavItem {
    id: string;
    label: string;
    icon: string;
  }

  const items: NavItem[] = [
    { id: "status", label: "Статус", icon: "●" },
    { id: "strategies", label: "Стратегии", icon: "⚙" },
    { id: "filters", label: "Фильтры", icon: "▦" },
    { id: "updates", label: "Обновления", icon: "↑" },
    { id: "diagnostics", label: "Диагностика", icon: "✚" },
    { id: "settings", label: "Настройки", icon: "★" },
  ];

  let { active, onNavigate } = $props<{ active: string; onNavigate: (t: string) => void }>();
</script>

<aside
  class="flex w-[220px] shrink-0 flex-col bg-ink-800 border-r border-ink-700"
>
  <div class="flex items-center gap-2 px-4 h-14 border-b border-ink-700">
    <div
      class="h-7 w-7 rounded-md bg-gradient-to-br from-brand-400 to-brand-700 flex items-center justify-center text-white font-bold text-sm"
    >
      Z
    </div>
    <div class="leading-tight">
      <div class="text-sm font-semibold text-ink-100">Zapret GUI</div>
      <div class="text-[10px] text-ink-400">DPI bypass controller</div>
    </div>
  </div>

  <nav class="flex-1 py-3 px-2 space-y-1">
    {#each items as item (item.id)}
      <button
        class="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors text-left
          {active === item.id
          ? 'bg-brand-600/15 text-brand-200 border border-brand-600/30'
          : 'text-ink-200 hover:bg-ink-700 border border-transparent'}"
        onclick={() => onNavigate(item.id)}
      >
        <span class="text-base w-4 text-center">{item.icon}</span>
        <span>{item.label}</span>
      </button>
    {/each}
  </nav>

  <div class="px-3 py-3 border-t border-ink-700 text-[10px] text-ink-400">
    v1.0.0 · MIT
  </div>
</aside>
