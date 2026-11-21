import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---------- types mirrored from Rust ----------

export interface AppConfig {
  active_strategy: string;
  game_filter: boolean;
  ipset_mode: string;
  auto_update_check: boolean;
  update_interval_hours: number;
  minimize_to_tray: boolean;
  theme: string;
  zapret_bin_version: string;
}

export interface Preset {
  id: string;
  name: string;
  category: string;
  description: string;
  args: string[];
  recommended: boolean;
}

export interface Presets {
  version: number;
  presets: Preset[];
}

export type ServiceState =
  | "stopped"
  | "startpending"
  | "running"
  | "stoppending"
  | "continuepending"
  | "pausepending"
  | "paused"
  | "unknown";

export interface ServiceSnapshot {
  installed: boolean;
  state: string;
  pid: number | null;
  start_type: string | null;
  bin_path: string | null;
}

export interface ProcSnapshot {
  running: boolean;
  pid: number | null;
  preset_id: string | null;
  started_at: string | null;
  uptime_secs: number | null;
}

export interface StatusEvent {
  installed: boolean;
  state: string;
  pid: number | null;
  adhoc_running: boolean;
  adhoc_pid: number | null;
  adhoc_preset: string | null;
  adhoc_uptime_secs: number | null;
  health_ok: boolean;
  health_latency_ms: number | null;
}

export interface FilterState {
  game_filter: boolean;
  ipset_mode: string;
  ipset_label: string;
  needs_restart: boolean;
  dirty: boolean;
}

export interface UpdateCheckResult {
  has_update: boolean;
  latest: ReleaseInfo | null;
  current_version: string;
  message: string;
}

export interface ReleaseInfo {
  tag_name: string;
  name: string;
  published_at: string;
  html_url: string;
  body: string;
  assets: AssetInfo[];
}

export interface AssetInfo {
  name: string;
  size: number;
  browser_download_url: string;
  content_type: string;
}

export interface TestResult {
  preset_id: string;
  preset_name: string;
  success: boolean;
  latency_ms: number | null;
  error: string | null;
}

export interface TestReport {
  started_at: string;
  finished_at: string;
  targets: string[];
  results: TestResult[];
  best_preset: string | null;
}

export type Outcome = "ok" | "warn" | "fail" | "info";

export interface DiagnosticItem {
  id: string;
  title: string;
  outcome: Outcome;
  detail: string;
  hint: string | null;
}

export interface DiagnosticReport {
  items: DiagnosticItem[];
  passed: number;
  warned: number;
  failed: number;
}

// ---------- command wrappers ----------

export const api = {
  getConfig: () => invoke<AppConfig>("get_config"),
  saveConfig: (state: AppConfig) => invoke<void>("save_config", { state }),
  setActiveStrategy: (strategyId: string) =>
    invoke<void>("set_active_strategy", { strategyId }),

  serviceSnapshot: () => invoke<ServiceSnapshot>("service_snapshot"),
  serviceInstall: () => invoke<void>("service_install"),
  serviceRemove: () => invoke<void>("service_remove"),
  serviceStart: () => invoke<void>("service_start"),
  serviceStop: () => invoke<void>("service_stop"),
  isElevated: () => invoke<boolean>("is_elevated"),

  listPresets: () => invoke<Presets>("list_presets"),
  defaultTargets: () => invoke<string[]>("default_targets"),
  resolvePresetArgs: (presetId: string) =>
    invoke<string[]>("resolve_preset_args", { presetId }),
  runStrategyTests: (
    presetIds: string[] | null,
    targets: string[] | null
  ) =>
    invoke<TestReport>("run_strategy_tests", {
      presetIds,
      targets,
    }),

  filterState: () => invoke<FilterState>("filter_state"),
  setGameFilter: (enabled: boolean) =>
    invoke<void>("set_game_filter", { enabled }),
  setIpsetMode: (mode: string) => invoke<void>("set_ipset_mode", { mode }),

  processSnapshot: () => invoke<ProcSnapshot>("process_snapshot"),
  processStart: (presetId: string | null) =>
    invoke<number>("process_start", { presetId }),
  processStop: () => invoke<void>("process_stop"),
  binExists: () => invoke<boolean>("bin_exists"),

  checkUpstreamUpdate: () =>
    invoke<UpdateCheckResult>("check_upstream_update"),
  updateZapretBinaries: () => invoke<string>("update_zapret_binaries"),
  refreshStrategies: () => invoke<number>("refresh_strategies"),
  updateIpsetList: () => invoke<void>("update_ipset_list"),
  updateHostsFile: () => invoke<void>("update_hosts_file"),

  runDiagnostics: () => invoke<DiagnosticReport>("run_diagnostics"),
  clearDiscordCache: () => invoke<number>("clear_discord_cache"),
  openPath: (path: string) => invoke<void>("open_path", { path }),
};

// ---------- event helpers ----------

export function onStatus(cb: (e: StatusEvent) => void): Promise<UnlistenFn> {
  return listen<StatusEvent>("service://status", (ev) => cb(ev.payload));
}

export function onConfig(cb: (c: AppConfig) => void): Promise<UnlistenFn> {
  return listen<AppConfig>("config://updated", (ev) => cb(ev.payload));
}

export function onLog(
  cb: (l: { ts: string; level: string; message: string }) => void
): Promise<UnlistenFn> {
  return listen<{ ts: string; level: string; message: string }>(
    "app://log",
    (ev) => cb(ev.payload)
  );
}

export function onTestsFinished(cb: (r: TestReport) => void): Promise<UnlistenFn> {
  return listen<TestReport>("tests://finished", (ev) => cb(ev.payload));
}

export function onUpdaterAvailable(
  cb: (r: UpdateCheckResult) => void
): Promise<UnlistenFn> {
  return listen<UpdateCheckResult>("updater://available", (ev) =>
    cb(ev.payload)
  );
}

export function onTrayToggle(cb: () => void): Promise<UnlistenFn> {
  return listen("tray://toggle", () => cb());
}
