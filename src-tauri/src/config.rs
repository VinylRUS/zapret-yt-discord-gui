//! Configuration persistence (config.toml) and runtime paths.
//!
//! All persistent state — `bin/`, `lists/`, `config.toml`, `presets.json` —
//! lives **next to the executable** (the install folder), NOT in `%APPDATA%`.
//! Only the runtime logs go to `%APPDATA%\zapret-gui\logs` so they survive a
//! reinstall and stay user-writable even if the install folder is under
//! `Program Files`. The zapret binaries (winws.exe, WinDivert, fake TLS
//! files) and the lists are downloaded into the install folder at first run,
//! so the installer stays tiny.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static APP_DIR: OnceLock<PathBuf> = OnceLock::new();
static LOGS_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Logical name of the Windows service backing zapret.
pub const SERVICE_NAME: &str = "Zapret";

/// Sub-directory that holds winws.exe + WinDivert + fake files.
pub const BIN_DIR_NAME: &str = "bin";
/// Sub-directory that holds list-*.txt and ipset-*.txt.
pub const LISTS_DIR_NAME: &str = "lists";
/// Sub-directory for runtime logs.
pub const LOGS_DIR_NAME: &str = "logs";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Name of the currently selected strategy preset.
    #[serde(default = "default_strategy")]
    pub active_strategy: String,

    /// Game Filter toggle (UDP/TCP on ports > 1023).
    #[serde(default)]
    pub game_filter: bool,

    /// IPSet filter mode: none | loaded | any.
    #[serde(default = "default_ipset_mode")]
    pub ipset_mode: String,

    /// Automatically check for updates on startup.
    #[serde(default = "default_true")]
    pub auto_update_check: bool,

    /// Update interval in hours.
    #[serde(default = "default_update_hours")]
    pub update_interval_hours: u32,

    /// Minimise to tray on close instead of quitting.
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,

    /// Theme: "dark" | "light".
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Version of the bundled zapret binaries (semantic-ish).
    #[serde(default)]
    pub zapret_bin_version: String,
}

fn default_strategy() -> String {
    "general".to_string()
}
fn default_ipset_mode() -> String {
    "none".to_string()
}
fn default_true() -> bool {
    true
}
fn default_update_hours() -> u32 {
    6
}
fn default_theme() -> String {
    "dark".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            active_strategy: default_strategy(),
            game_filter: false,
            ipset_mode: default_ipset_mode(),
            auto_update_check: true,
            update_interval_hours: default_update_hours(),
            minimize_to_tray: true,
            theme: default_theme(),
            zapret_bin_version: String::new(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let p = config_path();
        if !p.exists() {
            let cfg = Self::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let s = std::fs::read_to_string(&p)
            .with_context(|| format!("read config {}", p.display()))?;
        let cfg: Self = toml::from_str(&s).context("parse config.toml")?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let p = config_path();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let s = toml::to_string_pretty(self).context("serialize config")?;
        std::fs::write(&p, s).with_context(|| format!("write {}", p.display()))?;
        Ok(())
    }
}

/// Returns the application data directory: the folder that holds the
/// executable (the install folder). All non-log data lives here. Computed
/// once from `current_exe()`; falls back to the current working directory
/// if the exe path can't be resolved (e.g. some unit-test harnesses).
pub fn app_dir() -> &'static Path {
    APP_DIR
        .get_or_init(|| {
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                });
            exe_dir
        })
        .as_path()
}

/// Logs go to `%APPDATA%\zapret-gui\logs` (roaming, survives reinstall,
/// always user-writable). Everything else lives in [`app_dir`].
pub fn logs_dir() -> PathBuf {
    LOGS_DIR
        .get_or_init(|| {
            let base = dirs::data_dir()
                .or_else(|| dirs::config_dir())
                .unwrap_or_else(|| PathBuf::from("."));
            base.join("zapret-gui").join(LOGS_DIR_NAME)
        })
        .clone()
}

pub fn bin_dir() -> PathBuf {
    app_dir().join(BIN_DIR_NAME)
}
pub fn lists_dir() -> PathBuf {
    app_dir().join(LISTS_DIR_NAME)
}
pub fn config_path() -> PathBuf {
    app_dir().join("config.toml")
}
pub fn presets_path() -> PathBuf {
    app_dir().join("presets.json")
}

/// Path to the winws.exe executable inside the bundled bin dir.
pub fn winws_exe() -> PathBuf {
    bin_dir().join("winws.exe")
}

/// Ensure all expected sub-directories exist (bin/lists in the install
/// folder, logs in AppData).
pub fn ensure_dirs() -> Result<()> {
    for d in [app_dir(), &bin_dir(), &lists_dir()] {
        std::fs::create_dir_all(d).with_context(|| format!("mkdir {}", d.display()))?;
    }
    std::fs::create_dir_all(logs_dir()).with_context(|| format!("mkdir {}", logs_dir().display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let c = AppConfig::default();
        assert_eq!(c.active_strategy, "general");
        assert!(!c.game_filter);
        assert_eq!(c.ipset_mode, "none");
        assert!(c.auto_update_check);
        assert_eq!(c.theme, "dark");
    }

    #[test]
    fn roundtrip_toml() {
        let mut c = AppConfig::default();
        c.active_strategy = "general (ALT3)".into();
        c.game_filter = true;
        c.ipset_mode = "loaded".into();
        let s = toml::to_string(&c).unwrap();
        let back: AppConfig = toml::from_str(&s).unwrap();
        assert_eq!(back.active_strategy, "general (ALT3)");
        assert!(back.game_filter);
        assert_eq!(back.ipset_mode, "loaded");
    }

    #[test]
    fn parses_minimal_toml() {
        let s = r#"active_strategy = "fake"
ipset_mode = "any"
"#;
        let c: AppConfig = toml::from_str(s).unwrap();
        assert_eq!(c.active_strategy, "fake");
        assert_eq!(c.ipset_mode, "any");
        // defaults kept
        assert!(c.auto_update_check);
    }

    #[test]
    fn paths_are_inside_app_dir() {
        assert!(winws_exe().starts_with(app_dir()));
        assert!(lists_dir().starts_with(app_dir()));
        assert!(config_path().starts_with(app_dir()));
    }
}
