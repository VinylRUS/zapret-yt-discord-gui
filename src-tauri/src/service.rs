//! Windows Service Control Manager integration.
//!
//! Manages a Windows service named `Zapret` that runs `winws.exe` with
//! the currently selected strategy. On non-Windows the functions compile
//! to no-ops/errors so the rest of the code stays portable for `cargo test`.
//!
//! The service is created as `SERVICE_OWN_PROCESS` with auto-start, so it
//! boots with Windows even before any user logs in.

use crate::config;
use crate::strategy;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

pub const SERVICE_NAME: &str = config::SERVICE_NAME;
pub const SERVICE_DISPLAY: &str = "Zapret DPI Bypass";
pub const SERVICE_DESC: &str =
    "Bypasses DPI restrictions for Discord, YouTube and other blocked services.";

/// Coarse service status for the frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceState {
    Stopped,
    StartPending,
    Running,
    StopPending,
    ContinuePending,
    PausePending,
    Paused,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSnapshot {
    pub installed: bool,
    pub state: ServiceState,
    pub pid: Option<u32>,
    pub start_type: Option<String>,
    pub bin_path: Option<String>,
}

impl ServiceSnapshot {
    pub fn not_installed() -> Self {
        Self {
            installed: false,
            state: ServiceState::Unknown,
            pid: None,
            start_type: None,
            bin_path: None,
        }
    }
}

/// Build the full command line (exe + args) that the service will run.
/// Returned as a single quoted string suitable for SCM `binPath`.
pub fn build_service_command(cfg: &config::AppConfig) -> Result<String> {
    let presets = strategy::Presets::load()?;
    let preset = presets
        .find(&cfg.active_strategy)
        .ok_or_else(|| anyhow!("active strategy '{}' not found in presets.json", cfg.active_strategy))?;
    let args = strategy::build_launch_args(preset, cfg);
    let exe = config::winws_exe();
    if !exe.exists() {
        return Err(anyhow!(
            "winws.exe missing at {}. Use the Setup button on the Status tab first.",
            exe.display()
        ));
    }
    let mut cmd = format!("\"{}\"", exe.display());
    for a in args {
        cmd.push(' ');
        if a.contains(' ') || a.contains('=') == false {
            // winws args are usually `--flag=value` or `--flag value`;
            // we quote anything containing a space
            if a.contains(' ') {
                cmd.push('"');
                cmd.push_str(&a);
                cmd.push('"');
            } else {
                cmd.push_str(&a);
            }
        } else {
            cmd.push_str(&a);
        }
    }
    Ok(cmd)
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::ffi::OsString;
    use windows_service::service::{
        ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceState as WinState,
        ServiceType,
    };
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

    /// Parse a `"exe" arg arg --flag=value` command line (as produced by
    /// [`build_service_command`]) into the executable path and a list of
    /// launch arguments. Quoted tokens are unquoted; bare tokens are split
    /// on whitespace.
    fn split_command_line(command_line: &str) -> (std::path::PathBuf, Vec<OsString>) {
        let mut chars = command_line.trim().chars().peekable();
        let mut exe = String::new();
        if chars.peek() == Some(&'"') {
            chars.next();
            while let Some(&c) = chars.peek() {
                if c == '"' {
                    chars.next();
                    break;
                }
                exe.push(c);
                chars.next();
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                exe.push(c);
                chars.next();
            }
        }
        let mut args: Vec<OsString> = Vec::new();
        while chars.peek().is_some() {
            while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
                chars.next();
            }
            if chars.peek().is_none() {
                break;
            }
            let mut tok = String::new();
            if chars.peek() == Some(&'"') {
                chars.next();
                while let Some(&c) = chars.peek() {
                    if c == '"' {
                        chars.next();
                        break;
                    }
                    tok.push(c);
                    chars.next();
                }
            } else {
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() {
                        break;
                    }
                    tok.push(c);
                    chars.next();
                }
            }
            if !tok.is_empty() {
                args.push(OsString::from(tok));
            }
        }
        (std::path::PathBuf::from(exe), args)
    }

    pub fn install(command_line: &str) -> Result<()> {
        ensure_elevated()?;
        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
        )?;

        // if it already exists, delete first so binPath is refreshed
        let _ = remove_inner();

        let (exe_path, launch_args) = split_command_line(command_line);

        let info = ServiceInfo {
            name: OsString::from(SERVICE_NAME),
            display_name: OsString::from(SERVICE_DISPLAY),
            service_type: ServiceType::OWN_PROCESS,
            start_type: ServiceStartType::AutoStart,
            error_control: ServiceErrorControl::Normal,
            executable_path: exe_path,
            launch_arguments: launch_args,
            dependencies: vec![],
            account_name: None,
            account_password: None,
        };

        let _service = manager.create_service(
            &info,
            ServiceAccess::QUERY_STATUS | ServiceAccess::START | ServiceAccess::STOP | ServiceAccess::DELETE,
        )?;
        Ok(())
    }

    pub fn remove_inner() -> Result<()> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service(
            SERVICE_NAME,
            ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
        )?;
        // best-effort stop
        let _ = service.stop();
        // give SCM a moment to settle
        std::thread::sleep(std::time::Duration::from_millis(500));
        service.delete()?;
        Ok(())
    }

    pub fn remove() -> Result<()> {
        ensure_elevated()?;
        remove_inner()?;
        // also try to clean lingering WinDivert driver instances
        let _ = cleanup_windivert();
        Ok(())
    }

    pub fn start() -> Result<()> {
        ensure_elevated()?;
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service(SERVICE_NAME, ServiceAccess::START)?;
        service.start::<&str>(&[])?;
        Ok(())
    }

    pub fn stop() -> Result<()> {
        ensure_elevated()?;
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service(SERVICE_NAME, ServiceAccess::STOP)?;
        service.stop()?;
        Ok(())
    }

    pub fn snapshot() -> ServiceSnapshot {
        let manager = match ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) {
            Ok(m) => m,
            Err(_) => return ServiceSnapshot::not_installed(),
        };
        let service = match manager.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS) {
            Ok(s) => s,
            Err(_) => return ServiceSnapshot::not_installed(),
        };
        let status = match service.query_status() {
            Ok(s) => s,
            Err(_) => return ServiceSnapshot::not_installed(),
        };
        ServiceSnapshot {
            installed: true,
            state: map_state(status.current_state),
            pid: status.process_id,
            start_type: Some(format!("{:?}", status.service_type)),
            bin_path: None,
        }
    }

    fn map_state(s: WinState) -> ServiceState {
        match s {
            WinState::Stopped => ServiceState::Stopped,
            WinState::StartPending => ServiceState::StartPending,
            WinState::Running => ServiceState::Running,
            WinState::StopPending => ServiceState::StopPending,
            WinState::ContinuePending => ServiceState::ContinuePending,
            WinState::PausePending => ServiceState::PausePending,
            WinState::Paused => ServiceState::Paused,
            #[allow(unreachable_patterns)]
            _ => ServiceState::Unknown,
        }
    }

    /// Kill any lingering WinDivert driver services. Mirrors the upstream
    /// `driverquery | find "Divert"` advice.
    pub fn cleanup_windivert() -> Result<()> {
        let out = std::process::Command::new("driverquery")
            .output()?;
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.to_ascii_lowercase().contains("divert") {
                // the service name is typically the first token after the
                // header; we just attempt sc stop / sc delete on candidates
                let name = line.split_whitespace().next().unwrap_or("");
                if !name.is_empty() {
                    let _ = std::process::Command::new("sc")
                        .args(["stop", name])
                        .output();
                    let _ = std::process::Command::new("sc")
                        .args(["delete", name])
                        .output();
                }
            }
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;
    pub fn install(_: &str) -> Result<()> {
        Err(anyhow!("service management is only available on Windows"))
    }
    pub fn remove() -> Result<()> {
        Err(anyhow!("service management is only available on Windows"))
    }
    pub fn remove_inner() -> Result<()> {
        Err(anyhow!("service management is only available on Windows"))
    }
    pub fn start() -> Result<()> {
        Err(anyhow!("service management is only available on Windows"))
    }
    pub fn stop() -> Result<()> {
        Err(anyhow!("service management is only available on Windows"))
    }
    pub fn snapshot() -> ServiceSnapshot {
        ServiceSnapshot::not_installed()
    }
    pub fn cleanup_windivert() -> Result<()> {
        Ok(())
    }
}

/// Public API — thin wrappers so the rest of the crate does not need to
/// care about the cfg split.
pub fn install_service(cfg: &config::AppConfig) -> Result<()> {
    let cmd = build_service_command(cfg)?;
    platform::install(&cmd)?;
    // start it right away so the user sees immediate effect
    let _ = platform::start();
    Ok(())
}

pub fn remove_service() -> Result<()> {
    platform::remove()
}

pub fn start_service() -> Result<()> {
    platform::start()
}

pub fn stop_service() -> Result<()> {
    platform::stop()
}

pub fn snapshot() -> ServiceSnapshot {
    platform::snapshot()
}

pub fn cleanup_windivert() -> Result<()> {
    platform::cleanup_windivert()
}

/// True when the current process token is elevated (admin). On non-Windows
/// always returns `true` so tests pass. On Windows we use the `net session`
/// probe, which only succeeds under an elevated token — it avoids pulling
/// in fragile token-API signatures that drift between `windows` crate
/// versions.
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("net")
            .args(["session"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        matches!(out, Ok(s) if s.success())
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Bail if not elevated, with an actionable message.
pub fn ensure_elevated() -> Result<()> {
    if !is_elevated() {
        return Err(anyhow!(
            "this action requires administrator privileges — relaunch the app as administrator"
        ));
    }
    Ok(())
}

/// Encode a Rust string into a PCWSTR-compatible raw buffer. Used by the
/// windows-service crate shim above.
#[allow(dead_code)]
fn _encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_installed_snapshot() {
        let s = ServiceSnapshot::not_installed();
        assert!(!s.installed);
        assert_eq!(s.state, ServiceState::Unknown);
    }

    #[test]
    fn build_service_command_missing_winws_errors() {
        // winws.exe will not exist in test env; expect an error mentioning Setup
        let cfg = config::AppConfig::default();
        let r = build_service_command(&cfg);
        assert!(r.is_err());
        let msg = r.unwrap_err().to_string().to_ascii_lowercase();
        assert!(msg.contains("winws.exe") || msg.contains("missing"));
    }

    #[test]
    fn state_is_serialisable() {
        let s = ServiceState::Running;
        let v = serde_json::to_string(&s).unwrap();
        assert_eq!(v, "\"running\"");
    }

    #[test]
    fn elevated_on_non_windows() {
        // on non-Windows we always report elevated so logic compiles & tests run
        if cfg!(not(windows)) {
            assert!(is_elevated());
        }
    }
}
