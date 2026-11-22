//! Real-time service monitor.
//!
//! A Tauri async task polls the SCM service status every few seconds and
//! emits a `service://status` event to all windows. The frontend
//! subscribes once and renders the green/red dot in the top bar.
//!
//! We also run a tiny health probe (HTTPS HEAD to a target) so the UI can
//! show "обход работает / не работает" rather than just "сервис запущен".

use crate::config;
use crate::process;
use crate::service;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Interval between status polls.
const POLL_INTERVAL: Duration = Duration::from_secs(3);
/// Interval between health probes.
const HEALTH_INTERVAL: Duration = Duration::from_secs(15);
/// URL we probe to confirm the bypass is actually working.
const HEALTH_URL: &str = "https://www.youtube.com";

/// Last-known health result, shared between the periodic monitor loop and
/// [`build_status`] so an on-demand emit (right after start/stop) doesn't
/// lose the health dot.
static LAST_HEALTH: Lazy<Mutex<(bool, Option<u64>)>> =
    Lazy::new(|| Mutex::new((false, None)));

#[derive(Debug, Clone, Serialize)]
pub struct StatusEvent {
    pub installed: bool,
    pub state: String,
    pub pid: Option<u32>,
    pub adhoc_running: bool,
    pub adhoc_pid: Option<u32>,
    pub adhoc_preset: Option<String>,
    pub adhoc_uptime_secs: Option<u64>,
    pub health_ok: bool,
    pub health_latency_ms: Option<u64>,
}

/// Build a status event from the current service + process snapshots and
/// the cached health probe result. Shared by the periodic loop and
/// [`emit_status_now`] so a start/stop reflects in the UI instantly instead
/// of waiting up to `POLL_INTERVAL` for the next tick.
pub fn build_status() -> StatusEvent {
    let snap = service::snapshot();
    let proc_snap = process::snapshot();
    let (health_ok, health_lat) = *LAST_HEALTH.lock().unwrap();
    StatusEvent {
        installed: snap.installed,
        state: format!("{:?}", snap.state).to_ascii_lowercase(),
        pid: snap.pid,
        adhoc_running: proc_snap.running,
        adhoc_pid: proc_snap.pid,
        adhoc_preset: proc_snap.preset_id,
        adhoc_uptime_secs: proc_snap.uptime_secs,
        health_ok,
        health_latency_ms: health_lat,
    }
}

/// Emit a status event right now (used after start/stop/config changes so
/// the frontend updates without waiting for the next monitor tick).
pub fn emit_status_now(app: &AppHandle) {
    let _ = app.emit("service://status", &build_status());
}

/// Spawn the monitor background task. Call once during app setup.
///
/// Uses `tauri::async_runtime::spawn`, NOT `tokio::spawn`: the latter
/// requires an entered Tokio runtime context, which is NOT active on the
/// main thread during `setup`. Calling `tokio::spawn` from there panics
/// with "there is no reactor running". `tauri::async_runtime::spawn` runs
/// the future on Tauri's managed runtime, so the `tokio::time::sleep` and
/// `tokio::task::spawn_blocking` calls inside the block work correctly.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut tick = 0u64;
        loop {
            // run health probe every HEALTH_INTERVAL ticks
            if tick % (HEALTH_INTERVAL.as_secs() / POLL_INTERVAL.as_secs()).max(1) == 0 {
                match tokio::task::spawn_blocking(|| {
                    crate::strategy::probe(HEALTH_URL, Duration::from_secs(6))
                })
                .await
                {
                    Ok(Ok(ms)) => {
                        *LAST_HEALTH.lock().unwrap() = (true, Some(ms));
                    }
                    _ => {
                        *LAST_HEALTH.lock().unwrap() = (false, None);
                    }
                }
            }

            let evt = build_status();
            let _ = app.emit("service://status", &evt);

            tick = tick.wrapping_add(1);
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
}

/// Emit a one-off log line event to the frontend log panel.
pub fn emit_log(app: &AppHandle, level: &str, message: &str) {
    #[derive(Serialize)]
    struct LogLine<'a> {
        ts: String,
        level: &'a str,
        message: &'a str,
    }
    let _ = app.emit(
        "app://log",
        &LogLine {
            ts: chrono::Utc::now().to_rfc3339(),
            level,
            message,
        },
    );
}

/// Load the current config and emit it as a `config://updated` event so
/// the frontend can refresh its state.
pub fn emit_config(app: &AppHandle) {
    if let Ok(cfg) = config::AppConfig::load() {
        let _ = app.emit("config://updated", &cfg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_are_reasonable() {
        assert!(POLL_INTERVAL.as_secs() <= 10);
        assert!(HEALTH_INTERVAL.as_secs() >= POLL_INTERVAL.as_secs());
    }

    #[test]
    fn status_event_field_names_stable() {
        // make sure the serialised keys match what the frontend reads
        let e = StatusEvent {
            installed: false,
            state: "stopped".into(),
            pid: None,
            adhoc_running: false,
            adhoc_pid: None,
            adhoc_preset: None,
            adhoc_uptime_secs: None,
            health_ok: false,
            health_latency_ms: None,
        };
        let s = serde_json::to_string(&e).unwrap();
        for key in [
            "installed",
            "state",
            "pid",
            "adhoc_running",
            "adhoc_pid",
            "adhoc_preset",
            "adhoc_uptime_secs",
            "health_ok",
            "health_latency_ms",
        ] {
            assert!(s.contains(&format!("\"{key}\"")), "missing key {key}");
        }
    }
}
