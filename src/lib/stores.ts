import { writable, derived, type Readable } from "svelte/store";
import {
  api,
  onStatus,
  onConfig,
  onLog,
  onTrayToggle,
  type AppConfig,
  type Presets,
  type StatusEvent,
} from "./api";

export const status = writable<StatusEvent | null>(null);
export const config = writable<AppConfig | null>(null);
export const presets = writable<Presets | null>(null);
export const elevated = writable<boolean>(false);
export const logs = writable<{ ts: string; level: string; message: string }[]>(
  []
);
export const busy = writable<Record<string, boolean>>({});

export const serviceRunning: Readable<boolean> = derived(
  status,
  ($s) => $s?.state === "running" || $s?.adhoc_running === true
);

export const healthOk: Readable<boolean> = derived(
  status,
  ($s) => $s?.health_ok ?? false
);

export function setBusy(key: string, value: boolean) {
  busy.update((b) => ({ ...b, [key]: value }));
}

export function pushLog(line: { ts: string; level: string; message: string }) {
  logs.update((l) => [...l.slice(-199), line]);
}

export async function refreshAll() {
  try {
    const [cfg, presetsVal, isElevated] = await Promise.all([
      api.getConfig(),
      api.listPresets(),
      api.isElevated(),
    ]);
    config.set(cfg);
    presets.set(presetsVal);
    elevated.set(isElevated);
  } catch (e) {
    pushLog({ ts: new Date().toISOString(), level: "error", message: String(e) });
  }
}

let initialised = false;
export async function initStores() {
  if (initialised) return;
  initialised = true;

  await refreshAll();

  onStatus((e) => status.set(e));
  onConfig((c) => config.set(c));
  onLog((l) => pushLog(l));
  onTrayToggle(async () => {
    const s = await api.serviceSnapshot();
    if (s.state === "running") {
      await api.serviceStop().catch(() => {});
    } else {
      await api.serviceStart().catch(() => {});
    }
  });
}
