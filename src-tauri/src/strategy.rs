//! Strategy presets and diagnostic strategy testing.
//!
//! A preset is a named combination of `winws.exe` arguments. The set
//! mirrors the original `general*.bat` family from
//! Flowseal/zapret-discord-youtube. Tokens in the argument list are
//! substituted at launch time:
//!
//! * `{LISTS}` — absolute path to the lists directory
//! * `{BIN}`   — absolute path to the bin directory
//! * `{IPSET}` — `ipset-all.txt` path (shortcut for `{LISTS}/ipset-all.txt`)
//!
//! The diagnostic flow (`run_tests`) boots each preset, probes a list of
//! targets over HTTPS, then stops the process and records whether the
//! probe succeeded. Results are returned as a JSON-serialisable report.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use crate::config;

/// One preset definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    /// winws.exe arguments (tokens will be substituted).
    pub args: Vec<String>,
    /// Recommended for first try.
    #[serde(default)]
    pub recommended: bool,
}

/// Container persisted as `presets.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Presets {
    pub version: u32,
    pub presets: Vec<Preset>,
}

/// One row of a strategy test report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub preset_id: String,
    pub preset_name: String,
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

/// Aggregated report returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    pub started_at: String,
    pub finished_at: String,
    pub targets: Vec<String>,
    pub results: Vec<TestResult>,
    pub best_preset: Option<String>,
}

impl Presets {
    /// Load from disk, falling back to the built-in defaults and saving
    /// them so the user can edit the file later.
    pub fn load() -> Result<Self> {
        let p = config::presets_path();
        if !p.exists() {
            let def = Self::defaults();
            def.save()?;
            return Ok(def);
        }
        let s = std::fs::read_to_string(&p)
            .with_context(|| format!("read presets {}", p.display()))?;
        let parsed: Self = serde_json::from_str(&s).context("parse presets.json")?;
        Ok(parsed)
    }

    pub fn save(&self) -> Result<()> {
        let p = config::presets_path();
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let s = serde_json::to_string_pretty(self)?;
        std::fs::write(&p, s)?;
        Ok(())
    }

    pub fn find(&self, id: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.id == id)
    }

    /// Built-in default preset set, mirroring the upstream `general*.bat`.
    pub fn defaults() -> Self {
        Self {
            version: 1,
            presets: default_presets(),
        }
    }
}

/// Substitute path tokens in an argument list.
pub fn resolve_args(args: &[String]) -> Vec<String> {
    let lists = config::lists_dir().to_string_lossy().to_string();
    let bin = config::bin_dir().to_string_lossy().to_string();
    let ipset = config::lists_dir().join("ipset-all.txt").to_string_lossy().to_string();
    args.iter()
        .map(|a| {
            a.replace("{LISTS}", &lists)
                .replace("{BIN}", &bin)
                .replace("{IPSET}", &ipset)
        })
        .collect()
}

/// The list of URLs we probe during diagnostics. Mirrors the spirit of
/// the upstream `utils/targets.txt`.
pub fn default_targets() -> Vec<String> {
    vec![
        "https://www.youtube.com".into(),
        "https://discord.com".into(),
        "https://gateway.discord.gg".into(),
        "https://github.com".into(),
    ]
}

/// Probe a single URL with a short timeout. Returns latency in ms on
/// success. We do NOT trust system DNS to be healthy (it may be exactly
/// what zapret is fixing), so we resolve via reqwest's default resolver
/// and accept whatever it gives back; the goal is a fast TLS handshake.
pub fn probe(url: &str, timeout: Duration) -> std::result::Result<u64, String> {
    let start = std::time::Instant::now();
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().map_err(|e| e.to_string())?;
    let _ = resp.error_for_status_ref();
    Ok(start.elapsed().as_millis() as u64)
}

/// Run all presets sequentially, returning a report. This is intended to
/// be invoked from a background Tokio task — it is synchronous and may
/// take several minutes. Each preset is started as a child process, given
/// time to settle, then probed against every target.
#[allow(dead_code)]
pub fn run_tests(presets: &Presets, targets: &[String]) -> Result<TestReport> {
    use std::process::Command;

    let started_at = chrono::Utc::now().to_rfc3339();
    let winws = config::winws_exe();
    if !winws.exists() {
        anyhow::bail!(
            "winws.exe not found at {}. Run Setup first.",
            winws.display()
        );
    }

    let mut results = Vec::with_capacity(presets.presets.len());
    let mut best: Option<(String, u64)> = None;

    for preset in &presets.presets {
        let args = resolve_args(&preset.args);
        log::info!("testing preset {} ({})", preset.id, preset.name);

        let mut cmd = Command::new(&winws);
        cmd.args(&args);
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                results.push(TestResult {
                    preset_id: preset.id.clone(),
                    preset_name: preset.name.clone(),
                    success: false,
                    latency_ms: None,
                    error: Some(format!("spawn failed: {e}")),
                });
                continue;
            }
        };

        // let the strategy settle before probing
        std::thread::sleep(Duration::from_secs(4));

        let mut ok = false;
        let mut latency: Option<u64> = None;
        let mut err: Option<String> = None;

        for t in targets {
            match probe(t, Duration::from_secs(8)) {
                Ok(ms) => {
                    ok = true;
                    latency = Some(match latency {
                        Some(prev) if prev < ms => prev,
                        _ => ms,
                    });
                }
                Err(e) => {
                    err = Some(e);
                }
            }
        }

        // clean up the child
        let _ = kill_child(child);

        results.push(TestResult {
            preset_id: preset.id.clone(),
            preset_name: preset.name.clone(),
            success: ok,
            latency_ms: latency,
            error: if ok { None } else { err },
        });

        if ok {
            if let Some(ms) = latency {
                match &best {
                    Some((_, b)) if *b <= ms => {}
                    _ => best = Some((preset.id.clone(), ms)),
                }
            } else if best.is_none() {
                best = Some((preset.id.clone(), u64::MAX));
            }
        }

        // small pause between presets so WinDivert releases the filter
        std::thread::sleep(Duration::from_millis(800));
    }

    let best_preset = best.map(|(id, _)| id);
    Ok(TestReport {
        started_at,
        finished_at: chrono::Utc::now().to_rfc3339(),
        targets: targets.to_vec(),
        results,
        best_preset,
    })
}

#[cfg(windows)]
fn kill_child(mut child: std::process::Child) -> std::io::Result<()> {
    // windows: taskkill /F /T /PID is the robust way to take down
    // winws + the WinDivert handle it holds
    let pid = child.id();
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/T", "/PID"])
        .arg(pid.to_string())
        .output();
    let _ = child.wait();
    Ok(())
}

#[cfg(not(windows))]
fn kill_child(mut child: std::process::Child) -> std::io::Result<()> {
    use std::io::ErrorKind;
    // best-effort: SIGTERM then reap
    unsafe {
        libc_kill(child.id() as i32, 15);
    }
    match child.wait() {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::InvalidInput => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(not(windows))]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}
#[cfg(not(windows))]
unsafe fn libc_kill(pid: i32, sig: i32) {
    let _ = kill(pid, sig);
}

/// Build the default preset set. Categories mirror the upstream family:
/// `general`, `alt`, `fake-tls-auto`, `simple-fake`, `exp`.
///
/// These are honest approximations of the upstream `general*.bat` argument
/// sets; the upstream files frequently tweak parameters, so when the
/// bundled `bin/` is updated the user may also refresh `presets.json` from
/// the "Updates" tab.
fn default_presets() -> Vec<Preset> {
    vec![
        Preset {
            id: "general".into(),
            name: "General (default)".into(),
            category: "general".into(),
            description: "Стандартная стратегия из general.bat. Рекомендуется первой.".into(),
            args: vec![
                "--wf-tcp=443-65535".into(),
                "--wf-udp=443-65535,50000-65535".into(),
                "--filter-tcp=443".into(),
                "--filter-l7={LISTS}/list-general-user.txt".into(),
                "--filter-udp=443".into(),
                "--dpi-desync=fake".into(),
                "--dpi-desync-repeats=6".into(),
                "--dpi-desync-fake-sni={BIN}/fake_sni".into(),
            ],
            recommended: true,
        },
        Preset {
            id: "general-alt".into(),
            name: "General (ALT)".into(),
            category: "alt".into(),
            description: "Альтернативные параметры fake/disorder.".into(),
            args: vec![
                "--wf-tcp=443-65535".into(),
                "--wf-udp=443-65535,50000-65535".into(),
                "--filter-tcp=443".into(),
                "--filter-l7={LISTS}/list-general-user.txt".into(),
                "--filter-udp=443".into(),
                "--dpi-desync=disorder2".into(),
                "--dpi-desync-repeats=6".into(),
                "--dpi-desync-fake-sni={BIN}/fake_sni".into(),
            ],
            recommended: false,
        },
        Preset {
            id: "general-alt2".into(),
            name: "General (ALT2)".into(),
            category: "alt".into(),
            description: "split2 + fake.".into(),
            args: vec![
                "--wf-tcp=443-65535".into(),
                "--wf-udp=443-65535,50000-65535".into(),
                "--filter-tcp=443".into(),
                "--filter-l7={LISTS}/list-general-user.txt".into(),
                "--filter-udp=443".into(),
                "--dpi-desync=split2".into(),
                "--dpi-desync-repeats=6".into(),
                "--dpi-desync-fake-sni={BIN}/fake_sni".into(),
            ],
            recommended: false,
        },
        Preset {
            id: "general-alt3".into(),
            name: "General (ALT3)".into(),
            category: "alt".into(),
            description: "fake + ttl + disorder.".into(),
            args: vec![
                "--wf-tcp=443-65535".into(),
                "--wf-udp=443-65535,50000-65535".into(),
                "--filter-tcp=443".into(),
                "--filter-l7={LISTS}/list-general-user.txt".into(),
                "--filter-udp=443".into(),
                "--dpi-desync=fake".into(),
                "--dpi-desync-ttl=5".into(),
                "--dpi-desync-repeats=6".into(),
                "--dpi-desync-fake-sni={BIN}/fake_sni".into(),
            ],
            recommended: false,
        },
        Preset {
            id: "general-fake-tls-auto".into(),
            name: "General (FAKE TLS AUTO)".into(),
            category: "fake-tls-auto".into(),
            description: "Авто-подбор fake TLS fingerprint.".into(),
            args: vec![
                "--wf-tcp=443-65535".into(),
                "--wf-udp=443-65535,50000-65535".into(),
                "--filter-tcp=443".into(),
                "--filter-l7={LISTS}/list-general-user.txt".into(),
                "--filter-udp=443".into(),
                "--dpi-desync=fake".into(),
                "--dpi-desync-repeats=6".into(),
                "--dpi-desync-fake-sni=www.google.com".into(),
                "--dpi-desync-fake-sni-extra={BIN}/fake_sni_extra".into(),
            ],
            recommended: false,
        },
        Preset {
            id: "general-fake-tls-auto-alt".into(),
            name: "General (FAKE TLS AUTO ALT)".into(),
            category: "fake-tls-auto".into(),
            description: "Вариация fake TLS auto.".into(),
            args: vec![
                "--wf-tcp=443-65535".into(),
                "--wf-udp=443-65535,50000-65535".into(),
                "--filter-tcp=443".into(),
                "--filter-l7={LISTS}/list-general-user.txt".into(),
                "--filter-udp=443".into(),
                "--dpi-desync=fake".into(),
                "--dpi-desync-repeats=8".into(),
                "--dpi-desync-fake-sni=www.iana.org".into(),
            ],
            recommended: false,
        },
        Preset {
            id: "general-simple-fake".into(),
            name: "General (SIMPLE FAKE)".into(),
            category: "simple-fake".into(),
            description: "Упрощённый fake без авто-подбора.".into(),
            args: vec![
                "--wf-tcp=443-65535".into(),
                "--wf-udp=443-65535,50000-65535".into(),
                "--filter-tcp=443".into(),
                "--filter-l7={LISTS}/list-general-user.txt".into(),
                "--dpi-desync=fake".into(),
                "--dpi-desync-repeats=4".into(),
                "--dpi-desync-fake-sni={BIN}/fake_sni".into(),
            ],
            recommended: false,
        },
        Preset {
            id: "general-simple-fake-alt".into(),
            name: "General (SIMPLE FAKE ALT)".into(),
            category: "simple-fake".into(),
            description: "Упрощённый fake, альтернатива.".into(),
            args: vec![
                "--wf-tcp=443-65535".into(),
                "--wf-udp=443-65535,50000-65535".into(),
                "--filter-tcp=443".into(),
                "--filter-l7={LISTS}/list-general-user.txt".into(),
                "--dpi-desync=disorder2".into(),
                "--dpi-desync-repeats=4".into(),
                "--dpi-desync-fake-sni={BIN}/fake_sni".into(),
            ],
            recommended: false,
        },
        Preset {
            id: "general-exp".into(),
            name: "General (EXP)".into(),
            category: "exp".into(),
            description: "Экспериментальная стратегия. Может ломать часть трафика.".into(),
            args: vec![
                "--wf-tcp=443-65535".into(),
                "--wf-udp=443-65535,50000-65535".into(),
                "--filter-tcp=443".into(),
                "--filter-l7={LISTS}/list-general-user.txt".into(),
                "--filter-udp=443".into(),
                "--dpi-desync=fake,split2,disorder2".into(),
                "--dpi-desync-repeats=10".into(),
                "--dpi-desync-fake-sni={BIN}/fake_sni".into(),
            ],
            recommended: false,
        },
        Preset {
            id: "general-discord-only".into(),
            name: "General (Discord only)".into(),
            category: "general".into(),
            description: "Только домены Discord + UDP 443 (голос).".into(),
            args: vec![
                "--wf-udp=443,50000-65535".into(),
                "--filter-udp=443".into(),
                "--filter-l7={LISTS}/list-discord.txt".into(),
                "--dpi-desync=fake".into(),
                "--dpi-desync-repeats=6".into(),
                "--dpi-desync-fake-sni={BIN}/fake_sni".into(),
            ],
            recommended: false,
        },
    ]
}

/// Build the full argument list for a preset given the current config
/// (applies Game/IPSet filter overlays on top of the base args).
///
/// Flowseal strategies embed `{GFTCP}` / `{GFUDP}` tokens where the game-filter
/// port list goes (mirroring `%GameFilterTCP%` / `%GameFilterUDP%` in the
/// upstream `.bat`). When `game_filter` is on we substitute the default flowseal
/// range `1024-65535`; when off we substitute `12` (a harmless placeholder
/// port, exactly like flowseal's disabled mode) so the `--wf-tcp=...,12` line
/// stays syntactically valid.
pub fn build_launch_args(preset: &Preset, cfg: &config::AppConfig) -> Vec<String> {
    let mut base = resolve_args(&preset.args);

    let gf_ports = if cfg.game_filter { "1024-65535" } else { "12" };
    base = base
        .iter()
        .map(|a| a.replace("{GFTCP}", gf_ports).replace("{GFUDP}", gf_ports))
        // strip a trailing comma left if a token somehow ended up empty
        .map(|a| a.trim_end_matches(',').to_string())
        .collect();

    match cfg.ipset_mode.as_str() {
        "loaded" => {
            base.push("--ipset={LISTS}/ipset-all.txt".into());
            base = resolve_args(&base);
        }
        "any" => {
            base.push("--ipset=any".into());
        }
        _ => {}
    }

    base
}

/// Resolve a preset's full path inside the loaded presets file.
pub fn preset_path_for(id: &str) -> Result<PathBuf> {
    let p = Presets::load()?;
    if p.find(id).is_none() {
        anyhow::bail!("preset '{id}' not found");
    }
    Ok(config::presets_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_general() {
        let p = Presets::defaults();
        assert!(p.presets.iter().any(|x| x.id == "general"));
        assert!(p.presets.iter().any(|x| x.category == "fake-tls-auto"));
        assert!(p.presets.len() >= 8);
    }

    #[test]
    fn resolve_args_substitutes_tokens() {
        let args = vec![
            "--filter-l7={LISTS}/list.txt".to_string(),
            "--fake={BIN}/fake_sni".to_string(),
            "--ipset={IPSET}".to_string(),
        ];
        let r = resolve_args(&args);
        assert!(!r[0].contains("{LISTS}"));
        assert!(r[0].ends_with("list.txt"));
        assert!(!r[1].contains("{BIN}"));
        assert!(r[2].ends_with("ipset-all.txt"));
    }

    #[test]
    fn game_filter_overlay_substitutes_tokens() {
        // flowseal strategies carry {GFTCP}/{GFUDP} tokens; with game_filter
        // on they must resolve to the default range, off to the placeholder.
        let preset = Preset {
            id: "test".into(),
            name: "test".into(),
            category: "general".into(),
            description: String::new(),
            args: vec![
                "--wf-tcp=80,443,{GFTCP}".into(),
                "--wf-udp=443,{GFUDP}".into(),
            ],
            recommended: false,
        };
        let mut cfg_on = config::AppConfig::default();
        cfg_on.game_filter = true;
        let args_on = build_launch_args(&preset, &cfg_on);
        assert!(args_on.iter().any(|a| a == "--wf-tcp=80,443,1024-65535"));
        assert!(args_on.iter().any(|a| a == "--wf-udp=443,1024-65535"));

        let cfg_off = config::AppConfig::default();
        let args_off = build_launch_args(&preset, &cfg_off);
        assert!(args_off.iter().any(|a| a == "--wf-tcp=80,443,12"));
        assert!(args_off.iter().any(|a| a == "--wf-udp=443,12"));
    }

    #[test]
    fn ipset_loaded_adds_ipset_arg() {
        let preset = Presets::defaults().find("general").unwrap().clone();
        let mut cfg = config::AppConfig::default();
        cfg.ipset_mode = "loaded".into();
        let args = build_launch_args(&preset, &cfg);
        assert!(args.iter().any(|a| a.starts_with("--ipset=")));
    }

    #[test]
    fn ipset_any_adds_any() {
        let preset = Presets::defaults().find("general").unwrap().clone();
        let mut cfg = config::AppConfig::default();
        cfg.ipset_mode = "any".into();
        let args = build_launch_args(&preset, &cfg);
        assert!(args.iter().any(|a| a == "--ipset=any"));
    }

    #[test]
    fn presets_roundtrip() {
        let p = Presets::defaults();
        let s = serde_json::to_string(&p).unwrap();
        let back: Presets = serde_json::from_str(&s).unwrap();
        assert_eq!(back.presets.len(), p.presets.len());
    }

    #[test]
    fn find_returns_some() {
        let p = Presets::defaults();
        assert!(p.find("general").is_some());
        assert!(p.find("does-not-exist").is_none());
    }
}
