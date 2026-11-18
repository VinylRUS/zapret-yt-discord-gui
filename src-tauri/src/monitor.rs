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
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Interval between status polls.
const POLL_INTERVAL: Duration = Duration::from_secs(3);
/// Interval between health probes.
const HEALTH_INTERVAL: Duration = Duration::from_secs(15);
/// URL we probe to confirm the bypass is actually working.
const HEALTH_URL: &str = "https://www.youtube.com";

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

/// Spawn the monitor background task. Call once during app setup.
pub fn spawn(app: AppHandle) {
    tokio::spawn(async move {
        let mut tick = 0u64;
        let mut last_health_ok = false;
        let mut last_health_lat: Option<u64> = None;
        loop {
            let snap = service::snapshot();
            let proc_snap = process::snapshot();

            // run health probe every HEALTH_INTERVAL ticks
            if tick % (HEALTH_INTERVAL.as_secs() / POLL_INTERVAL.as_secs()).max(1) == 0 {
                match tokio::task::spawn_blocking(|| {
                    crate::strategy::probe(HEALTH_URL, Duration::from_secs(6))
                })
                .await
                {
                    Ok(Ok(ms)) => {
                        last_health_ok = true;
                        last_health_lat = Some(ms);
                    }
                    _ => {
                        last_health_ok = false;
                        last_health_lat = None;
                    }
                }
            }

            let evt = StatusEvent {
                installed: snap.installed,
                state: format!("{:?}", snap.state).to_ascii_lowercase(),
                pid: snap.pid,
                adhoc_running: proc_snap.running,
                adhoc_pid: proc_snap.pid,
                adhoc_preset: proc_snap.preset_id,
                adhoc_uptime_secs: proc_snap.uptime_secs,
                health_ok: last_health_ok,
                health_latency_ms: last_health_lat,
            };
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
