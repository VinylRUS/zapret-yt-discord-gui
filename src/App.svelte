<script lang="ts">
  import { onMount } from "svelte";
  import { initStores, status, elevated } from "./lib/stores";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Topbar from "./lib/components/Topbar.svelte";
  import Status from "./lib/pages/Status.svelte";
  import Strategies from "./lib/pages/Strategies.svelte";
  import Filters from "./lib/pages/Filters.svelte";
  import Updates from "./lib/pages/Updates.svelte";
  import Diagnostics from "./lib/pages/Diagnostics.svelte";
  import Settings from "./lib/pages/Settings.svelte";

  type TabId =
    | "status"
    | "strategies"
    | "filters"
    | "updates"
    | "diagnostics"
    | "settings";

  let active: TabId = $state("status");

  onMount(() => {
    initStores();
  });
</script>

<div class="flex h-screen w-screen bg-ink-900 text-ink-100">
  <Sidebar {active}
    onNavigate={(t: TabId) => (active = t)} />
  <div class="flex flex-1 flex-col min-w-0">
    <Topbar />
    {#if !$elevated}
      <div class="bg-amber-500/10 border-b border-amber-500/30 text-amber-200 px-6 py-2 text-sm">
        Приложение запущено без прав администратора. Управление сервисом WinDivert требует UAC —
        перезапустите от имени администратора.
      </div>
    {/if}
    <main class="flex-1 overflow-y-auto">
      {#if active === "status"}
        <Status />
      {:else if active === "strategies"}
        <Strategies />
      {:else if active === "filters"}
        <Filters />
      {:else if active === "updates"}
        <Updates />
      {:else if active === "diagnostics"}
        <Diagnostics />
      {:else if active === "settings"}
        <Settings />
      {/if}
    </main>
  </div>
</div>
