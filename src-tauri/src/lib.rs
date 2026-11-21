//! Zapret GUI — Tauri 2 backend entry.
//!
//! All modules are private helpers; this file wires them into Tauri
//! commands that the Svelte frontend invokes via `@tauri-apps/api/core`
//! `invoke`.

pub mod config;
pub mod crash;
pub mod diagnostics;
pub mod filters;
pub mod monitor;
pub mod process;
pub mod service;
pub mod strategy;
pub mod updater;

use config::AppConfig;
use tauri::{Emitter, Manager};

// ---------------------------------------------------------------------------
// Config commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_config() -> Result<AppConfig, String> {
    AppConfig::load().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_config(state: AppConfig) -> Result<(), String> {
    state.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_active_strategy(strategy_id: String) -> Result<(), String> {
    let mut cfg = AppConfig::load().map_err(|e| e.to_string())?;
    // verify preset exists
    let presets = strategy::Presets::load().map_err(|e| e.to_string())?;
    if presets.find(&strategy_id).is_none() {
        return Err(format!("preset '{strategy_id}' not found"));
    }
    cfg.active_strategy = strategy_id;
    cfg.save().map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Service commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn service_snapshot() -> Result<service::ServiceSnapshot, String> {
    Ok(service::snapshot())
}

#[tauri::command]
fn service_install() -> Result<(), String> {
    let cfg = AppConfig::load().map_err(|e| e.to_string())?;
    service::install_service(&cfg).map_err(|e| e.to_string())
}

#[tauri::command]
fn service_remove() -> Result<(), String> {
    service::remove_service().map_err(|e| e.to_string())
}

#[tauri::command]
fn service_start() -> Result<(), String> {
    service::start_service().map_err(|e| e.to_string())
}

#[tauri::command]
fn service_stop() -> Result<(), String> {
    service::stop_service().map_err(|e| e.to_string())
}

#[tauri::command]
fn is_elevated() -> bool {
    service::is_elevated()
}

// ---------------------------------------------------------------------------
// Strategy / preset commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_presets() -> Result<strategy::Presets, String> {
    strategy::Presets::load().map_err(|e| e.to_string())
}

#[tauri::command]
fn default_targets() -> Vec<String> {
    strategy::default_targets()
}

#[tauri::command]
fn resolve_preset_args(preset_id: String) -> Result<Vec<String>, String> {
    let cfg = AppConfig::load().map_err(|e| e.to_string())?;
    let presets = strategy::Presets::load().map_err(|e| e.to_string())?;
    let preset = presets
        .find(&preset_id)
        .ok_or_else(|| format!("preset '{preset_id}' not found"))?;
    Ok(strategy::build_launch_args(preset, &cfg))
}

/// Kick off a strategy test run. This is a long task — we spawn it on a
/// background thread and stream progress as `tests://progress` events.
#[tauri::command]
async fn run_strategy_tests(
    app: tauri::AppHandle,
    preset_ids: Option<Vec<String>>,
    targets: Option<Vec<String>>,
) -> Result<strategy::TestReport, String> {
    let app_clone = app.clone();
    let presets = strategy::Presets::load().map_err(|e| e.to_string())?;
    let presets = if let Some(ids) = preset_ids {
        let filtered: Vec<_> = presets
            .presets
            .into_iter()
            .filter(|p| ids.contains(&p.id))
            .collect();
        strategy::Presets {
            version: presets.version,
            presets: filtered,
        }
    } else {
        presets
    };
    let targets = targets.unwrap_or_else(default_targets);

    let app_for_progress = app_clone.clone();
    let report = tokio::task::spawn_blocking(move || -> Result<strategy::TestReport, String> {
        // emit a "started" log
        monitor::emit_log(
            &app_for_progress,
            "info",
            &format!("starting strategy tests for {} preset(s)", presets.presets.len()),
        );
        let r = strategy::run_tests(&presets, &targets).map_err(|e| e.to_string())?;
        monitor::emit_log(
            &app_for_progress,
            "info",
            &format!(
                "tests done, best={:?}",
                r.best_preset
            ),
        );
        Ok(r)
    })
    .await
    .map_err(|e| e.to_string())??;

    let _ = app.emit("tests://finished", &report);
    Ok(report)
}

// ---------------------------------------------------------------------------
// Filter commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn filter_state() -> Result<filters::FilterState, String> {
    let cfg = AppConfig::load().map_err(|e| e.to_string())?;
    Ok(filters::FilterState::from_config(&cfg))
}

#[tauri::command]
fn set_game_filter(enabled: bool) -> Result<(), String> {
    let mut cfg = AppConfig::load().map_err(|e| e.to_string())?;
    filters::set_game_filter(&mut cfg, enabled).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_ipset_mode(mode: String) -> Result<(), String> {
    let mut cfg = AppConfig::load().map_err(|e| e.to_string())?;
    filters::set_ipset_mode(&mut cfg, &mode).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Process commands (ad-hoc winws run, no service)
// ---------------------------------------------------------------------------

#[tauri::command]
fn process_snapshot() -> Result<process::ProcSnapshot, String> {
    Ok(process::snapshot())
}

#[tauri::command]
fn process_start(preset_id: Option<String>) -> Result<u32, String> {
    let cfg = AppConfig::load().map_err(|e| e.to_string())?;
    let presets = strategy::Presets::load().map_err(|e| e.to_string())?;
    let id = preset_id.unwrap_or_else(|| cfg.active_strategy.clone());
    let preset = presets
        .find(&id)
        .ok_or_else(|| format!("preset '{id}' not found"))?;
    process::start_preset(preset, &cfg).map_err(|e| e.to_string())
}

#[tauri::command]
fn process_stop() -> Result<(), String> {
    process::stop().map_err(|e| e.to_string())
}

#[tauri::command]
fn bin_exists() -> bool {
    process::bin_exists()
}

// ---------------------------------------------------------------------------
// Update commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn check_upstream_update() -> Result<updater::UpdateCheckResult, String> {
    updater::check_upstream_update().map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_zapret_binaries(app: tauri::AppHandle) -> Result<String, String> {
    let app = app.clone();
    tokio::task::spawn_blocking(move || -> Result<String, String> {
        monitor::emit_log(&app, "info", "downloading zapret binaries…");
        let r = updater::update_zapret_binaries().map_err(|e| e.to_string())?;
        monitor::emit_log(&app, "info", &format!("updated zapret to {r}"));
        Ok(r)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn update_ipset_list() -> Result<(), String> {
    updater::update_ipset_list().map_err(|e| e.to_string())
}

#[tauri::command]
fn update_hosts_file() -> Result<(), String> {
    updater::update_hosts_file().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Diagnostics commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn run_diagnostics() -> Result<diagnostics::DiagnosticReport, String> {
    Ok(diagnostics::run_diagnostics())
}

#[tauri::command]
fn clear_discord_cache() -> Result<u64, String> {
    diagnostics::clear_discord_cache()
}

/// Open a folder in the system file explorer (uses the shell plugin).
//
// `Shell::open` is deprecated in tauri-plugin-shell v2 in favour of the
// separate `tauri-plugin-opener` crate. We keep the shell plugin here to
// avoid pulling in another dependency + capability; the call still works.
#[allow(deprecated)]
#[tauri::command]
async fn open_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_shell::ShellExt;
    app.shell().open(path, None).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// App setup
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Install the crash hook BEFORE anything else: env_logger and the
    // builder below can panic (e.g. on bad config), and we want every
    // panic to produce a visible MessageBox + panic.log instead of a
    // silent process exit.
    crash::install_panic_hook();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // ensure data dirs exist
            if let Err(e) = config::ensure_dirs() {
                log::warn!("ensure_dirs failed: {e}");
            }
            // seed presets + config on first run
            if let Err(e) = strategy::Presets::load() {
                log::warn!("presets load failed: {e}");
            }
            if let Err(e) = config::AppConfig::load() {
                log::warn!("config load failed: {e}");
            }

            // spawn the realtime status monitor
            let handle = app.handle().clone();
            monitor::spawn(handle.clone());

            // emit initial config to the frontend
            monitor::emit_config(&handle);

            // tray menu
            #[cfg(desktop)]
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::TrayIconBuilder;
                let toggle = MenuItem::with_id(&handle, "toggle", "Start/Stop", true, None::<&str>)?;
                let show = MenuItem::with_id(&handle, "show", "Show window", true, None::<&str>)?;
                let quit = MenuItem::with_id(&handle, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(&handle, &[&toggle, &show, &quit])?;
                let mut builder = TrayIconBuilder::with_id("main")
                    .tooltip("Zapret GUI")
                    .menu(&menu)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        "toggle" => {
                            // emit a frontend event so it can run its own start/stop logic
                            let _ = app.emit("tray://toggle", ());
                        }
                        _ => {}
                    });
                // Don't `.unwrap()` the window icon — if embedding failed at
                // build time, a panic here would silently kill the app on
                // startup. Fall back to a tray without a custom icon instead.
                if let Some(icon) = handle.default_window_icon() {
                    builder = builder.icon(icon.clone());
                } else {
                    log::warn!("no default window icon embedded; tray will use the OS default");
                }
                builder.build(&handle)?;
            }

            // auto update check on startup if enabled
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let cfg = match config::AppConfig::load() {
                    Ok(c) => c,
                    Err(_) => return,
                };
                if !cfg.auto_update_check {
                    return;
                }
                match tokio::task::spawn_blocking(updater::check_upstream_update).await {
                    Ok(Ok(r)) => {
                        if r.has_update {
                            let _ = h.emit("updater://available", &r);
                        }
                    }
                    Ok(Err(e)) => log::warn!("update check failed: {e}"),
                    Err(e) => log::warn!("update check task failed: {e}"),
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // minimise to tray instead of quitting
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if let Ok(cfg) = config::AppConfig::load() {
                    if cfg.minimize_to_tray {
                        let _ = window.hide();
                        api.prevent_close();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            set_active_strategy,
            service_snapshot,
            service_install,
            service_remove,
            service_start,
            service_stop,
            is_elevated,
            list_presets,
            default_targets,
            resolve_preset_args,
            run_strategy_tests,
            filter_state,
            set_game_filter,
            set_ipset_mode,
            process_snapshot,
            process_start,
            process_stop,
            bin_exists,
            check_upstream_update,
            update_zapret_binaries,
            update_ipset_list,
            update_hosts_file,
            run_diagnostics,
            clear_discord_cache,
            open_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
