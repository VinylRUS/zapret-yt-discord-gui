//! Fetch and parse strategy `.bat` files from
//! [`Flowseal/zapret-discord-youtube`](https://github.com/Flowseal/zapret-discord-youtube).
//!
//! The flowseal repo ships one `general*.bat` per strategy at the repo root.
//! Each `.bat` ends with a single
//!
//! ```text
//! start "zapret: %~n0" /min "%BIN%winws.exe" <args> ^
//! <more args> ^
//! ...
//! <last args>
//! ```
//!
//! invocation, continued across lines with a trailing `^`. We extract the
//! `winws.exe` arguments, normalise the batch tokens (`%BIN%`, `%LISTS%`,
//! `%GameFilterTCP%`, `%GameFilterUDP%`, the `^!` escape) into our own
//! `{BIN}` / `{LISTS}` / `{GFTCP}` / `{GFUDP}` tokens, and turn each `.bat`
//! into a [`crate::strategy::Preset`]. At launch time
//! [`crate::strategy::build_launch_args`] resolves `{BIN}`/`{LISTS}` to the
//! app data dirs and `{GFTCP}`/`{GFUDP}` to the game-filter port range.
//!
//! This is the source of truth for presets — they are no longer hand-written
//! approximations. [`refresh_presets_from_flowseal`] re-pulls them on demand
//! (first run, "Update zapret binaries", and the "Обновить стратегии" button).

use crate::config;
use crate::strategy::{Preset, Presets};
use anyhow::{anyhow, Context, Result};

const CONTENTS_API: &str = "https://api.github.com/repos/Flowseal/zapret-discord-youtube/contents";
const RAW_BASE: &str = "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/HEAD";

/// Hardcoded fallback list of strategy `.bat` filenames, used when the GitHub
/// contents API is rate-limited (60 req/h unauthenticated). Mirrors the repo
/// root; sorted. If flowseal adds a new strategy it will still be picked up
/// when the API is reachable — this is only the offline fallback.
const KNOWN_BATS: &[&str] = &[
    "general.bat",
    "general (ALT).bat",
    "general (ALT2).bat",
    "general (ALT3).bat",
    "general (ALT4).bat",
    "general (ALT5).bat",
    "general (ALT6).bat",
    "general (ALT7).bat",
    "general (ALT8).bat",
    "general (ALT9).bat",
    "general (ALT10).bat",
    "general (ALT11).bat",
    "general (ALT12).bat",
    "general (ALT13).bat",
    "general (EXP).bat",
    "general (FAKE TLS AUTO).bat",
    "general (FAKE TLS AUTO ALT).bat",
    "general (FAKE TLS AUTO ALT2).bat",
    "general (FAKE TLS AUTO ALT3).bat",
    "general (SIMPLE FAKE).bat",
    "general (SIMPLE FAKE ALT).bat",
    "general (SIMPLE FAKE ALT2).bat",
];

fn http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent("zapret-gui/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow!("http client: {e}"))
}

/// Discover `general*.bat` filenames via the GitHub contents API. Falls back
/// to [`KNOWN_BATS`] on any error (rate limit, network, malformed JSON) so the
/// caller always gets a usable list.
pub fn list_strategy_bats() -> Vec<String> {
    let client = match http_client() {
        Ok(c) => c,
        Err(_) => return fallback_bats(),
    };
    let resp = match client.get(CONTENTS_API).send() {
        Ok(r) => r,
        Err(_) => return fallback_bats(),
    };
    if !resp.status().is_success() {
        log::warn!(
            "github contents api returned {}, using hardcoded bat list",
            resp.status()
        );
        return fallback_bats();
    }
    let v: serde_json::Value = match resp.json() {
        Ok(v) => v,
        Err(_) => return fallback_bats(),
    };
    let mut out: Vec<String> = Vec::new();
    if let Some(arr) = v.as_array() {
        for f in arr {
            if let Some(name) = f["name"].as_str() {
                let lower = name.to_ascii_lowercase();
                if lower.ends_with(".bat") && lower.starts_with("general") {
                    out.push(name.to_string());
                }
            }
        }
    }
    if out.is_empty() {
        return fallback_bats();
    }
    out.sort();
    out
}

fn fallback_bats() -> Vec<String> {
    KNOWN_BATS.iter().map(|s| s.to_string()).collect()
}

/// Percent-encode just the characters that appear in flowseal filenames
/// (space and parentheses) for use in a raw.githubusercontent.com URL.
fn encode_path_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b' ' => out.push_str("%20"),
            b'(' => out.push_str("%28"),
            b')' => out.push_str("%29"),
            _ => out.push(b as char),
        }
    }
    out
}

/// Fetch one `.bat` file's raw content from raw.githubusercontent.com.
pub fn fetch_raw(filename: &str) -> Result<String> {
    let url = format!("{}/{}", RAW_BASE, encode_path_segment(filename));
    let resp = http_client()?
        .get(&url)
        .send()
        .with_context(|| format!("fetch {filename}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("raw.githubusercontent {} -> {}", filename, resp.status()));
    }
    Ok(resp.text()?)
}

/// Windows-style tokenizer: split on whitespace outside double quotes,
/// dropping the quote characters themselves. Matches how `winws.exe` sees its
/// argv after the batch `start` line is parsed.
fn tokenize(s: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for ch in s.chars() {
        match ch {
            '"' => in_quote = !in_quote,
            c if c.is_whitespace() && !in_quote => {
                if !cur.is_empty() {
                    tokens.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

/// Turn a flowseal filename stem (e.g. `general (ALT3)`) into a stable id
/// (`general-alt3`).
fn slugify(stem: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Categorise a strategy by its name, mirroring the upstream `general*.bat`
/// family.
fn categorize(stem: &str) -> &'static str {
    let lower = stem.to_ascii_lowercase();
    if stem == "general" {
        "general"
    } else if lower.contains("fake tls auto") {
        "fake-tls-auto"
    } else if lower.contains("simple fake") {
        "simple-fake"
    } else if lower.contains("exp") {
        "exp"
    } else if lower.contains("alt") {
        "alt"
    } else {
        "general"
    }
}

fn description_for(stem: &str, category: &str) -> String {
    match category {
        "general" => "Стандартная стратегия из general.bat. Рекомендуется первой.".into(),
        "alt" => format!("Альтернативная стратегия {} из flowseal.", stem),
        "fake-tls-auto" => "Авто-подбор fake TLS fingerprint (из flowseal).".into(),
        "simple-fake" => "Упрощённый fake без авто-подбора (из flowseal).".into(),
        "exp" => "Экспериментальная стратегия из flowseal. Может ломать часть трафика.".into(),
        _ => format!("Стратегия {} из flowseal.", stem),
    }
}

/// Parse one `.bat` file into a [`Preset`].
pub fn parse_bat(filename: &str, content: &str) -> Result<Preset> {
    let lines: Vec<&str> = content.lines().collect();

    // locate the winws.exe invocation line
    let start_idx = lines
        .iter()
        .position(|l| l.contains("winws.exe"))
        .ok_or_else(|| anyhow!("no winws.exe line in {filename}"))?;

    let start_line = lines[start_idx];

    // take everything after the exe path. The exe is quoted
    // ("%BIN%winws.exe") so we split on the closing quote; fall back to a
    // space for an unquoted form.
    let after = if let Some((_, rest)) = start_line.split_once("winws.exe\"") {
        rest
    } else if let Some((_, rest)) = start_line.split_once("winws.exe ") {
        rest
    } else {
        return Err(anyhow!("could not locate winws.exe args in {filename}"));
    };

    // join `^`-continued lines
    let mut joined = after.to_string();
    let mut i = start_idx + 1;
    loop {
        let trimmed = joined.trim_end();
        if let Some(without) = trimmed.strip_suffix('^') {
            joined = without.to_string();
            if i < lines.len() {
                joined.push(' ');
                joined.push_str(lines[i]);
                i += 1;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    // inline batch escapes: `^!` -> `!`, then drop any stray caret
    let joined = joined.replace("^!", "!");
    let joined = joined.replace('^', "");

    // tokenize and substitute batch env tokens -> our launch-time tokens
    let raw_args = tokenize(&joined);
    let args: Vec<String> = raw_args
        .into_iter()
        .filter(|a| !a.is_empty())
        .map(|a| {
            a.replace("%BIN%", "{BIN}")
                .replace("%LISTS%", "{LISTS}")
                .replace("%GameFilterTCP%", "{GFTCP}")
                .replace("%GameFilterUDP%", "{GFUDP}")
        })
        .collect();

    if args.is_empty() {
        return Err(anyhow!("parsed zero args from {filename}"));
    }

    let stem = filename.strip_suffix(".bat").unwrap_or(filename);
    let id = slugify(stem);
    let category = categorize(stem).to_string();
    let description = description_for(stem, &category);

    Ok(Preset {
        id,
        name: stem.to_string(),
        category,
        description,
        args,
        recommended: stem == "general",
    })
}

/// Fetch every `general*.bat` from flowseal, parse them, and overwrite
/// `presets.json`. Returns the number of presets written. Preserves nothing
/// from the previous file — flowseal is the source of truth.
pub fn refresh_presets_from_flowseal() -> Result<usize> {
    let names = list_strategy_bats();
    log::info!("refreshing {} strategy bats from flowseal", names.len());

    let mut presets: Vec<Preset> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for name in &names {
        match fetch_raw(name).and_then(|c| parse_bat(name, &c)) {
            Ok(p) => presets.push(p),
            Err(e) => {
                log::warn!("skip strategy {name}: {e}");
                errors.push(name.clone());
            }
        }
    }

    if presets.is_empty() {
        return Err(anyhow!(
            "no strategies parsed from flowseal (tried {} bats, {} errors)",
            names.len(),
            errors.len()
        ));
    }

    let count = presets.len();
    let p = Presets {
        version: 2,
        presets,
    };
    p.save().context("save presets.json")?;
    log::info!(
        "wrote {} presets to {} ({} skipped)",
        count,
        config::presets_path().display(),
        errors.len()
    );
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_handles_parens_and_spaces() {
        assert_eq!(slugify("general"), "general");
        assert_eq!(slugify("general (ALT3)"), "general-alt3");
        assert_eq!(slugify("general (FAKE TLS AUTO ALT2)"), "general-fake-tls-auto-alt2");
    }

    #[test]
    fn categorize_matches_flowseal_families() {
        assert_eq!(categorize("general"), "general");
        assert_eq!(categorize("general (ALT3)"), "alt");
        assert_eq!(categorize("general (FAKE TLS AUTO)"), "fake-tls-auto");
        assert_eq!(categorize("general (SIMPLE FAKE ALT)"), "simple-fake");
        assert_eq!(categorize("general (EXP)"), "exp");
    }

    #[test]
    fn tokenize_drops_quotes_and_keeps_equals_paths() {
        let t = tokenize(r#"--hostlist="%LISTS%list-general.txt" --filter-udp=443"#);
        assert_eq!(t, vec![
            "--hostlist=%LISTS%list-general.txt".to_string(),
            "--filter-udp=443".to_string(),
        ]);
    }

    #[test]
    fn parse_bat_extracts_args_and_tokens() {
        let bat = "\
@echo off
set \"BIN=%~dp0bin\\\"\n\
set \"LISTS=%~dp0lists\\\"\n\
start \"zapret: %~n0\" /min \"%BIN%winws.exe\" --wf-tcp=80,443,%GameFilterTCP% --wf-udp=443,%GameFilterUDP% ^\n\
--filter-tcp=443 --hostlist=\"%LISTS%list-general.txt\" --dpi-desync=fake --dpi-desync-fake-tls=^! ^\n\
--filter-udp=443 --dpi-desync-fake-quic=\"%BIN%quic.bin\"\n\
";
        let p = parse_bat("general (ALT3).bat", bat).unwrap();
        assert_eq!(p.id, "general-alt3");
        assert_eq!(p.category, "alt");
        assert!(p.args.iter().any(|a| a == "--wf-tcp=80,443,{GFTCP}"));
        assert!(p.args.iter().any(|a| a == "--hostlist={LISTS}list-general.txt"));
        assert!(p.args.iter().any(|a| a == "--dpi-desync-fake-quic={BIN}quic.bin"));
        assert!(p.args.iter().any(|a| a == "--dpi-desync-fake-tls=!"));
        // no leftover batch tokens
        assert!(!p.args.iter().any(|a| a.contains('%') || a.contains('^')));
        assert!(!p.recommended);
    }

    #[test]
    fn parse_bat_general_is_recommended() {
        let bat = "start \"x\" /min \"%BIN%winws.exe\" --wf-tcp=443\n";
        let p = parse_bat("general.bat", bat).unwrap();
        assert_eq!(p.id, "general");
        assert!(p.recommended);
    }

    #[test]
    fn parse_bat_errors_without_winws() {
        let bat = "@echo off\nrem no winws here\n";
        assert!(parse_bat("general.bat", bat).is_err());
    }

    #[test]
    fn encode_path_segment_encodes_spaces_and_parens() {
        assert_eq!(
            encode_path_segment("general (ALT3).bat"),
            "general%20%28ALT3%29.bat"
        );
    }
}
