//! Ad-hoc `winws.exe` process management (non-service).
//!
//! Used by the manual "Start without service" button and by the
//! diagnostics/test runner. The service path (see [`crate::service`]) is
//! the recommended way to run zapret persistently; this module is for
//! short-lived foreground runs and for the test harness.

use crate::config;
use crate::strategy::{self, Preset};
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;

static RUNNING: Lazy<Mutex<Option<RunningProc>>> = Lazy::new(|| Mutex::new(None));

struct RunningProc {
    pid: u32,
    started_at: chrono::DateTime<chrono::Utc>,
    preset_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcSnapshot {
    pub running: bool,
    pub pid: Option<u32>,
    pub preset_id: Option<String>,
    pub started_at: Option<String>,
    pub uptime_secs: Option<u64>,
}

/// Spawn winws.exe with the given preset + current config. Fails if a
/// process is already running under this manager.
pub fn start_preset(preset: &Preset, cfg: &config::AppConfig) -> Result<u32> {
    {
        let guard = RUNNING.lock().unwrap();
        if guard.is_some() {
            return Err(anyhow!("a winws process is already running — stop it first"));
        }
    }
    let exe = config::winws_exe();
    if !exe.exists() {
        return Err(anyhow!(
            "winws.exe not found at {}. Run Setup first.",
            exe.display()
        ));
    }
    let args = strategy::build_launch_args(preset, cfg);
    log::info!("starting winws: {} {:?}", exe.display(), args);

    let mut cmd = Command::new(&exe);
    cmd.args(&args);
    cmd.current_dir(config::bin_dir());
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    let child = cmd.spawn()?;
    let pid = child.id();
    // detach: keep handle but don't wait
    std::mem::forget(child);

    let mut guard = RUNNING.lock().unwrap();
    *guard = Some(RunningProc {
        pid,
        started_at: chrono::Utc::now(),
        preset_id: preset.id.clone(),
    });
    Ok(pid)
}

/// Stop the process spawned via [`start_preset`]. On Windows we use
/// `taskkill /F /T /PID` so the WinDivert handle is released cleanly.
pub fn stop() -> Result<()> {
    let mut guard = RUNNING.lock().unwrap();
    if guard.is_none() {
        return Ok(());
    }
    let pid = guard.as_ref().unwrap().pid;
    let res = kill_pid(pid);
    *guard = None;
    res
}

pub fn snapshot() -> ProcSnapshot {
    let guard = RUNNING.lock().unwrap();
    match &*guard {
        Some(r) => {
            let uptime = (chrono::Utc::now() - r.started_at).num_seconds().max(0) as u64;
            // verify the pid is still alive; if not, clear the slot
            if !pid_alive(r.pid) {
                ProcSnapshot {
                    running: false,
                    pid: None,
                    preset_id: None,
                    started_at: None,
                    uptime_secs: None,
                }
            } else {
                ProcSnapshot {
                    running: true,
                    pid: Some(r.pid),
                    preset_id: Some(r.preset_id.clone()),
                    started_at: Some(r.started_at.to_rfc3339()),
                    uptime_secs: Some(uptime),
                }
            }
        }
        None => ProcSnapshot {
            running: false,
            pid: None,
            preset_id: None,
            started_at: None,
            uptime_secs: None,
        },
    }
}

pub fn bin_exists() -> bool {
    config::winws_exe().exists()
}

pub fn bin_path() -> PathBuf {
    config::winws_exe()
}

/// If an ad-hoc `winws.exe` process is currently running under this manager,
/// restart it with the given (already-saved) config so strategy / filter /
/// game-filter changes take effect immediately, without restarting the app.
/// If nothing is running, this is a no-op.
pub fn apply_config_live(cfg: &config::AppConfig) -> Result<()> {
    let running = RUNNING.lock().unwrap().is_some();
    if !running {
        return Ok(());
    }
    log::info!("restarting winws to apply new config");
    stop()?;
    // let WinDivert release the filter before we re-grab it
    std::thread::sleep(std::time::Duration::from_millis(600));
    let presets = crate::strategy::Presets::load()?;
    let preset = presets
        .find(&cfg.active_strategy)
        .ok_or_else(|| anyhow!("preset '{}' not found", cfg.active_strategy))?;
    start_preset(preset, cfg)?;
    Ok(())
}

#[cfg(windows)]
fn kill_pid(pid: u32) -> Result<()> {
    let out = Command::new("taskkill")
        .args(["/F", "/T", "/PID"])
        .arg(pid.to_string())
        .output()?;
    if !out.status.success() {
        log::warn!("taskkill rc={:?} stderr={}", out.status, String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}

#[cfg(not(windows))]
fn kill_pid(pid: u32) -> Result<()> {
    let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
    Ok(())
}

#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    // tasklist /FI "PID eq pid" returns success if the pid exists
    let out = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output();
    match out {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            s.contains(&pid.to_string())
        }
        Err(_) => false,
    }
}

#[cfg(not(windows))]
fn pid_alive(pid: u32) -> bool {
    let out = Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status();
    matches!(out, Ok(s) if s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_when_idle() {
        // we cannot assume the slot is empty across tests, so just check
        // the shape of the snapshot.
        let s = snapshot();
        assert!(!s.running || s.pid.is_some());
    }

    #[test]
    fn bin_exists_is_bool() {
        let _ = bin_exists();
    }

    #[test]
    fn bin_path_under_app_dir() {
        assert!(bin_path().starts_with(config::app_dir()));
    }
}
