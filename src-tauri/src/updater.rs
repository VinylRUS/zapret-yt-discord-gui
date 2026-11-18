//! Update checks against GitHub releases.
//!
//! Two distinct update flows:
//!
//! 1. **GUI self-update** — handled by `tauri-plugin-updater` using the
//!    endpoint in `tauri.conf.json`. This module only exposes a thin
//!    `check_self_update` helper so the frontend has a uniform API.
//!
//! 2. **zapret binary pack update** — downloads the latest release of
//!    Flowseal/zapret-discord-youtube, extracts `bin/` and `lists/` into
//!    our app data dir, and bumps `zapret_bin_version` in the config.
//!    This mirrors the upstream "Update IPSet List / Replace active fakes"
//!    buttons but in a single one-click flow.

use crate::config;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

const UPSTREAM_REPO: &str = "Flowseal/zapret-discord-youtube";
const UPSTREAM_API: &str = "https://api.github.com/repos/Flowseal/zapret-discord-youtube/releases/latest";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: String,
    pub published_at: String,
    pub html_url: String,
    pub body: String,
    pub assets: Vec<AssetInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetInfo {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub has_update: bool,
    pub latest: Option<ReleaseInfo>,
    pub current_version: String,
    pub message: String,
}

/// Query the GitHub API for the latest release of the upstream repo.
pub fn fetch_latest_release() -> Result<ReleaseInfo> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("zapret-gui/1.0")
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let resp = client.get(UPSTREAM_API).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!(
            "github api returned {}",
            resp.status()
        ));
    }
    let raw: serde_json::Value = resp.json()?;
    let tag = raw["tag_name"].as_str().unwrap_or("").to_string();
    let name = raw["name"].as_str().unwrap_or("").to_string();
    let published_at = raw["published_at"].as_str().unwrap_or("").to_string();
    let html_url = raw["html_url"].as_str().unwrap_or("").to_string();
    let body = raw["body"].as_str().unwrap_or("").to_string();
    let mut assets = Vec::new();
    if let Some(arr) = raw["assets"].as_array() {
        for a in arr {
            assets.push(AssetInfo {
                name: a["name"].as_str().unwrap_or("").to_string(),
                size: a["size"].as_u64().unwrap_or(0),
                browser_download_url: a["browser_download_url"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                content_type: a["content_type"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    Ok(ReleaseInfo {
        tag_name: tag,
        name,
        published_at,
        html_url,
        body,
        assets,
    })
}

/// Compare upstream tag to the stored bin version.
pub fn check_upstream_update() -> Result<UpdateCheckResult> {
    let cfg = config::AppConfig::load()?;
    let latest = fetch_latest_release()?;
    let has_update = latest.tag_name != cfg.zapret_bin_version && !latest.tag_name.is_empty();
    let message = if has_update {
        format!("Доступна новая версия zapret: {}", latest.tag_name)
    } else {
        "У вас последняя версия zapret.".to_string()
    };
    Ok(UpdateCheckResult {
        has_update,
        latest: Some(latest),
        current_version: cfg.zapret_bin_version.clone(),
        message,
    })
}

/// Pick the asset that looks like the main release zip. The upstream
/// project ships a single `*.zip` (or `*.7z`/`*.rar`); we prefer zip.
pub fn pick_release_asset<'a>(release: &'a ReleaseInfo) -> Result<&'a AssetInfo> {
    // prefer a .zip asset whose name contains "zapret" or "discord-youtube"
    let mut best: Option<&AssetInfo> = None;
    for a in &release.assets {
        let lower = a.name.to_ascii_lowercase();
        if lower.ends_with(".zip") {
            if lower.contains("zapret") || lower.contains("discord") || best.is_none() {
                best = Some(a);
            }
        }
    }
    best.or_else(|| release.assets.first())
        .ok_or_else(|| anyhow!("no release assets found"))
}

/// Download the latest upstream release and overlay its `bin/` and
/// `lists/` directories into our app data dir. Returns the new tag.
pub fn update_zapret_binaries() -> Result<String> {
    let release = fetch_latest_release()?;
    let asset = pick_release_asset(&release)?;
    log::info!("downloading {} ({} bytes)", asset.name, asset.size);

    let client = reqwest::blocking::Client::builder()
        .user_agent("zapret-gui/1.0")
        .timeout(std::time::Duration::from_secs(300))
        .build()?;
    let bytes = client
        .get(&asset.browser_download_url)
        .send()?
        .error_for_status()?
        .bytes()?;

    let tmp = std::env::temp_dir().join(format!("zapret-update-{}.zip", chrono::Utc::now().timestamp()));
    std::fs::write(&tmp, &bytes).context("write tmp zip")?;

    extract_zapret_zip(&tmp, &release.tag_name)?;
    let _ = std::fs::remove_file(&tmp);

    // persist version
    let mut cfg = config::AppConfig::load()?;
    cfg.zapret_bin_version = release.tag_name.clone();
    cfg.save()?;

    Ok(release.tag_name)
}

/// Extract a zapret release zip into the app data dir, overlaying `bin/`
/// and `lists/`. We accept either a flat zip (bin/ at root) or a nested
/// one (some-repo-name/bin/...).
pub fn extract_zapret_zip(zip_path: &std::path::Path, expected_tag: &str) -> Result<()> {
    let file = std::fs::File::open(zip_path).context("open zip")?;
    let mut archive = zip::ZipArchive::new(file).context("open zip archive")?;

    let bin_target = config::bin_dir();
    let lists_target = config::lists_dir();
    std::fs::create_dir_all(&bin_target)?;
    std::fs::create_dir_all(&lists_target)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        // normalise separators
        let norm = name.replace('\\', "/");
        let lower = norm.to_ascii_lowercase();

        // skip dirs
        if entry.is_dir() {
            continue;
        }

        // find "bin/" or "lists/" segment anywhere in the path
        let dest = if let Some(idx) = lower.find("/bin/") {
            Some(bin_target.join(&norm[idx + 5..]))
        } else if lower.starts_with("bin/") {
            Some(bin_target.join(&norm[4..]))
        } else if let Some(idx) = lower.find("/lists/") {
            Some(lists_target.join(&norm[idx + 7..]))
        } else if lower.starts_with("lists/") {
            Some(lists_target.join(&norm[6..]))
        } else {
            None
        };

        let Some(dest) = dest else { continue };
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        std::io::copy(&mut entry, &mut buf)?;
        std::fs::write(&dest, &buf).with_context(|| format!("write {}", dest.display()))?;
    }

    log::info!("extracted zapret {} into app data dir", expected_tag);
    Ok(())
}

/// Refresh just the ipset-all.txt list from the upstream repo (raw file).
pub fn update_ipset_list() -> Result<()> {
    let url = format!(
        "https://raw.githubusercontent.com/{}/main/lists/ipset-all.txt",
        UPSTREAM_REPO
    );
    let client = reqwest::blocking::Client::builder()
        .user_agent("zapret-gui/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let resp = client.get(&url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("raw.githubusercontent returned {}", resp.status()));
    }
    let text = resp.text()?;
    let target = config::lists_dir().join("ipset-all.txt");
    std::fs::write(&target, text)?;
    log::info!("ipset-all.txt refreshed at {}", target.display());
    Ok(())
}

/// Update the Windows hosts file to fix Telegram web / Discord voice.
/// Appends well-known mappings if they are missing. Requires elevation.
pub fn update_hosts_file() -> Result<()> {
    crate::service::ensure_elevated()?;
    let hosts = std::path::PathBuf::from(r"C:\Windows\System32\drivers\etc\hosts");
    if !hosts.exists() {
        return Err(anyhow!("hosts file not found at {}", hosts.display()));
    }
    let original = std::fs::read_to_string(&hosts)?;
    let additions = [
        "127.0.0.1 localhost",
        "# zapret-gui managed entries below",
        "149.154.175.50  web.telegram.org",
        "149.154.167.51  gateway.discord.gg",
    ];
    let mut new = original.clone();
    for line in additions {
        if !original.contains(line) {
            if !new.ends_with('\n') {
                new.push('\n');
            }
            new.push_str(line);
            new.push('\n');
        }
    }
    if new != original {
        std::fs::write(&hosts, new).context("write hosts (need admin)")?;
        log::info!("hosts file updated");
    } else {
        log::info!("hosts file already up to date");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_asset_prefers_zip() {
        let release = ReleaseInfo {
            tag_name: "v1".into(),
            name: "v1".into(),
            published_at: String::new(),
            html_url: String::new(),
            body: String::new(),
            assets: vec![
                AssetInfo {
                    name: "release.txt".into(),
                    size: 10,
                    browser_download_url: "http://x/txt".into(),
                    content_type: "text/plain".into(),
                },
                AssetInfo {
                    name: "zapret-discord-youtube.zip".into(),
                    size: 1000,
                    browser_download_url: "http://x/zip".into(),
                    content_type: "application/zip".into(),
                },
            ],
        };
        let a = pick_release_asset(&release).unwrap();
        assert!(a.name.ends_with(".zip"));
    }

    #[test]
    fn pick_asset_falls_back_to_first() {
        let release = ReleaseInfo {
            tag_name: "v1".into(),
            name: "v1".into(),
            published_at: String::new(),
            html_url: String::new(),
            body: String::new(),
            assets: vec![AssetInfo {
                name: "only.7z".into(),
                size: 10,
                browser_download_url: "http://x/7z".into(),
                content_type: "application/x-7z".into(),
            }],
        };
        let a = pick_release_asset(&release).unwrap();
        assert_eq!(a.name, "only.7z");
    }

    #[test]
    fn pick_asset_errors_when_empty() {
        let release = ReleaseInfo {
            tag_name: "v1".into(),
            name: "v1".into(),
            published_at: String::new(),
            html_url: String::new(),
            body: String::new(),
            assets: vec![],
        };
        assert!(pick_release_asset(&release).is_err());
    }

    #[test]
    fn hosts_additions_have_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for line in [
            "127.0.0.1 localhost",
            "# zapret-gui managed entries below",
            "149.154.175.50  web.telegram.org",
            "149.154.167.51  gateway.discord.gg",
        ] {
            assert!(seen.insert(line), "duplicate hosts addition: {line}");
        }
    }
}
