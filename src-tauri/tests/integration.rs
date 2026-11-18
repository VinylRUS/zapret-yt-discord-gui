//! Integration tests that exercise the public module API end-to-end
//! without booting the Tauri runtime. These run on any platform; the
//! Windows-only service/process paths are covered by `#[cfg(windows)]`
//! unit tests inside their modules.

use zapret_gui_lib::{config, diagnostics, filters, strategy, updater};

#[test]
fn config_default_roundtrips_through_toml() {
    let c = config::AppConfig::default();
    let s = toml::to_string(&c).expect("serialize");
    let back: config::AppConfig = toml::from_str(&s).expect("deserialize");
    assert_eq!(back.active_strategy, c.active_strategy);
    assert_eq!(back.game_filter, c.game_filter);
    assert_eq!(back.ipset_mode, c.ipset_mode);
}

#[test]
fn presets_have_at_least_one_recommended() {
    let p = strategy::Presets::defaults();
    assert!(
        p.presets.iter().any(|x| x.recommended),
        "expected at least one recommended preset"
    );
}

#[test]
fn preset_args_resolve_without_placeholders() {
    let p = strategy::Presets::defaults();
    for preset in &p.presets {
        let resolved = strategy::resolve_args(&preset.args);
        for arg in &resolved {
            assert!(
                !arg.contains("{LISTS}") && !arg.contains("{BIN}") && !arg.contains("{IPSET}"),
                "preset '{}' left a placeholder in arg: {arg}",
                preset.id
            );
        }
    }
}

#[test]
fn build_launch_args_none_ipset_does_not_add_ipset_flag() {
    let preset = strategy::Presets::defaults()
        .find("general")
        .expect("general preset")
        .clone();
    let mut cfg = config::AppConfig::default();
    cfg.ipset_mode = "none".into();
    let args = strategy::build_launch_args(&preset, &cfg);
    assert!(!args.iter().any(|a| a.starts_with("--ipset")));
}

#[test]
fn build_launch_args_loaded_adds_ipset_path() {
    let preset = strategy::Presets::defaults()
        .find("general")
        .expect("general preset")
        .clone();
    let mut cfg = config::AppConfig::default();
    cfg.ipset_mode = "loaded".into();
    let args = strategy::build_launch_args(&preset, &cfg);
    let ipset_arg = args
        .iter()
        .find(|a| a.starts_with("--ipset="))
        .expect("expected --ipset arg");
    assert!(ipset_arg.ends_with("ipset-all.txt"));
}

#[test]
fn filters_validate_modes() {
    assert!(filters::validate_ipset_mode("none").is_ok());
    assert!(filters::validate_ipset_mode("loaded").is_ok());
    assert!(filters::validate_ipset_mode("any").is_ok());
    assert!(filters::validate_ipset_mode("banana").is_err());
}

#[test]
fn diagnostics_report_runs() {
    let report = diagnostics::run_diagnostics();
    assert!(!report.items.is_empty());
    // every item has a non-empty title
    for item in &report.items {
        assert!(!item.title.is_empty(), "empty title in {:?}", item);
    }
}

#[test]
fn default_targets_include_youtube_and_discord() {
    let t = strategy::default_targets();
    let joined = t.join(" ");
    assert!(joined.contains("youtube.com"));
    assert!(joined.contains("discord.com"));
}

#[test]
fn updater_pick_asset_logic() {
    let release = updater::ReleaseInfo {
        tag_name: "v1".into(),
        name: "v1".into(),
        published_at: String::new(),
        html_url: String::new(),
        body: String::new(),
        assets: vec![
            updater::AssetInfo {
                name: "ignore.txt".into(),
                size: 1,
                browser_download_url: "u1".into(),
                content_type: "text/plain".into(),
            },
            updater::AssetInfo {
                name: "zapret-discord-youtube.zip".into(),
                size: 2,
                browser_download_url: "u2".into(),
                content_type: "application/zip".into(),
            },
        ],
    };
    let picked = updater::pick_release_asset(&release).expect("pick");
    assert_eq!(picked.name, "zapret-discord-youtube.zip");
}

#[test]
fn app_dir_paths_are_consistent() {
    assert!(config::winws_exe().starts_with(config::app_dir()));
    assert!(config::lists_dir().starts_with(config::app_dir()));
    assert!(config::presets_path().starts_with(config::app_dir()));
}
