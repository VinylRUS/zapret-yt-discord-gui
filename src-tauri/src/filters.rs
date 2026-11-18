//! Game Filter and IPSet Filter toggles.
//!
//! These two switches overlay extra arguments on top of the base strategy.
//! The actual argument injection happens in [`strategy::build_launch_args`];
//! this module owns the *semantics* — which values are valid, how the
//! toggles interact, and what the frontend should display.

use crate::config::AppConfig;
use serde::{Deserialize, Serialize};

/// The three IPSet filter modes, matching the upstream `service.bat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IpsetMode {
    /// No IPs are checked against the list.
    None,
    /// IPs are checked against `ipset-all.txt`.
    Loaded,
    /// All IPs are filtered. Side effects on many sites — not for permanent use.
    Any,
}

impl IpsetMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "loaded" => Some(Self::Loaded),
            "any" => Some(Self::Any),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Loaded => "loaded",
            Self::Any => "any",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Выключен",
            Self::Loaded => "По списку (ipset-all.txt)",
            Self::Any => "Все IP (не рекомендуется постоянно)",
        }
    }

    pub fn all() -> [Self; 3] {
        [Self::None, Self::Loaded, Self::Any]
    }
}

/// Human-readable description of the current filter state, used in the
/// status line of the Filters tab.
#[derive(Debug, Clone, Serialize)]
pub struct FilterState {
    pub game_filter: bool,
    pub ipset_mode: String,
    pub ipset_label: String,
    pub needs_restart: bool,
    /// True when the toggles differ from what the running service was
    /// launched with — flip this only after a restart is observed.
    pub dirty: bool,
}

impl FilterState {
    pub fn from_config(cfg: &AppConfig) -> Self {
        let mode = IpsetMode::parse(&cfg.ipset_mode).unwrap_or(IpsetMode::None);
        Self {
            game_filter: cfg.game_filter,
            ipset_mode: mode.as_str().to_string(),
            ipset_label: mode.label().to_string(),
            needs_restart: true,
            dirty: false,
        }
    }
}

/// Validate a raw IPSet string from the frontend. Returns the normalised
/// form or an error message.
pub fn validate_ipset_mode(raw: &str) -> Result<String, String> {
    match IpsetMode::parse(raw) {
        Some(m) => Ok(m.as_str().to_string()),
        None => Err(format!(
            "unknown ipset mode '{raw}', expected none|loaded|any"
        )),
    }
}

/// Apply the Game Filter toggle to the config in place and persist it.
pub fn set_game_filter(cfg: &mut AppConfig, enabled: bool) -> anyhow::Result<()> {
    if cfg.game_filter == enabled {
        return Ok(());
    }
    cfg.game_filter = enabled;
    cfg.save()?;
    log::info!("game_filter -> {enabled}");
    Ok(())
}

/// Apply the IPSet mode to the config in place and persist it.
pub fn set_ipset_mode(cfg: &mut AppConfig, mode: &str) -> anyhow::Result<()> {
    let norm = validate_ipset_mode(mode)?;
    if cfg.ipset_mode == norm {
        return Ok(());
    }
    cfg.ipset_mode = norm;
    cfg.save()?;
    log::info!("ipset_mode -> {norm}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_modes() {
        assert_eq!(IpsetMode::parse("none"), Some(IpsetMode::None));
        assert_eq!(IpsetMode::parse("LOADED"), Some(IpsetMode::Loaded));
        assert_eq!(IpsetMode::parse("Any"), Some(IpsetMode::Any));
        assert_eq!(IpsetMode::parse("bogus"), None);
    }

    #[test]
    fn validate_rejects_unknown() {
        assert!(validate_ipset_mode("none").is_ok());
        assert!(validate_ipset_mode("loaded").is_ok());
        assert!(validate_ipset_mode("any").is_ok());
        assert!(validate_ipset_mode("everything").is_err());
    }

    #[test]
    fn all_three_modes_present() {
        assert_eq!(IpsetMode::all().len(), 3);
    }

    #[test]
    fn state_from_config_labels_correctly() {
        let mut cfg = AppConfig::default();
        cfg.ipset_mode = "loaded".into();
        let st = FilterState::from_config(&cfg);
        assert!(st.ipset_label.contains("ipset-all"));
        cfg.ipset_mode = "any".into();
        let st = FilterState::from_config(&cfg);
        assert!(st.ipset_label.contains("не рекомендуется"));
    }

    #[test]
    fn set_ipset_mode_normalises_case() {
        let mut cfg = AppConfig::default();
        // we don't actually want to write to the real user dir during tests,
        // so just exercise the validation path.
        let norm = validate_ipset_mode("ANY").unwrap();
        assert_eq!(norm, "any");
        cfg.ipset_mode = norm;
        assert_eq!(cfg.ipset_mode, "any");
    }
}
