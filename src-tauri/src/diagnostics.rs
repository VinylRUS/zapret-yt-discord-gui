//! Diagnostics — mirror of the upstream `service.bat → Run Diagnostics`.
//!
//! Each check returns an outcome (`ok | warn | fail`) plus a hint. The
//! frontend renders the list; the user can then act on each item, e.g.
//! clear the Discord cache, enable Secure DNS, or fix the hosts file.

use crate::config;
use crate::process;
use crate::service;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Ok,
    Warn,
    Fail,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticItem {
    pub id: String,
    pub title: String,
    pub outcome: Outcome,
    pub detail: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub items: Vec<DiagnosticItem>,
    pub passed: usize,
    pub warned: usize,
    pub failed: usize,
}

pub fn run_diagnostics() -> DiagnosticReport {
    let mut items = Vec::new();
    items.push(check_winws_present());
    items.push(check_service_installed());
    items.push(check_service_running());
    items.push(check_windivert_driver());
    items.push(check_app_path_no_cyrillic());
    items.push(check_secure_dns_hint());
    items.push(check_hosts_file());
    items.push(check_discord_cache_present());
    items.push(check_adhoc_process());

    let passed = items.iter().filter(|i| i.outcome == Outcome::Ok).count();
    let warned = items.iter().filter(|i| i.outcome == Outcome::Warn).count();
    let failed = items.iter().filter(|i| i.outcome == Outcome::Fail).count();

    DiagnosticReport {
        items,
        passed,
        warned,
        failed,
    }
}

fn check_winws_present() -> DiagnosticItem {
    let exe = config::winws_exe();
    if exe.exists() {
        DiagnosticItem {
            id: "winws".into(),
            title: "winws.exe найден".into(),
            outcome: Outcome::Ok,
            detail: format!("Путь: {}", exe.display()),
            hint: None,
        }
    } else {
        DiagnosticItem {
            id: "winws".into(),
            title: "winws.exe отсутствует".into(),
            outcome: Outcome::Fail,
            detail: "Бинарник zapret не найден в каталоге приложения.".into(),
            hint: Some("Нажмите «Update zapret binaries» на вкладке Updates.".into()),
        }
    }
}

fn check_service_installed() -> DiagnosticItem {
    let snap = service::snapshot();
    if snap.installed {
        DiagnosticItem {
            id: "service-installed".into(),
            title: "Сервис установлен".into(),
            outcome: Outcome::Ok,
            detail: format!("Имя сервиса: {}", service::SERVICE_NAME),
            hint: None,
        }
    } else {
        DiagnosticItem {
            id: "service-installed".into(),
            title: "Сервис не установлен".into(),
            outcome: Outcome::Warn,
            detail: "Windows-сервис Zapret не зарегистрирован.".into(),
            hint: Some("Установите сервис на вкладке Status.".into()),
        }
    }
}

fn check_service_running() -> DiagnosticItem {
    let snap = service::snapshot();
    if !snap.installed {
        return DiagnosticItem {
            id: "service-running".into(),
            title: "Сервис не запущен".into(),
            outcome: Outcome::Info,
            detail: "Сервис не установлен — статус не определён.".into(),
            hint: None,
        };
    }
    match snap.state {
        service::ServiceState::Running => DiagnosticItem {
            id: "service-running".into(),
            title: "Сервис работает".into(),
            outcome: Outcome::Ok,
            detail: "Сервис zapret запущен.".into(),
            hint: None,
        },
        s => DiagnosticItem {
            id: "service-running".into(),
            title: "Сервис не запущен".into(),
            outcome: Outcome::Warn,
            detail: format!("Текущее состояние: {:?}", s),
            hint: Some("Нажмите «Start» на вкладке Status.".into()),
        },
    }
}

fn check_windivert_driver() -> DiagnosticItem {
    // query driver for the WinDivert service
    #[cfg(windows)]
    {
        let out = std::process::Command::new("driverquery")
            .arg("/FO")
            .arg("CSV")
            .output();
        match out {
            Ok(o) => {
                let text = String::from_utf8_lossy(&o.stdout);
                if text.to_ascii_lowercase().contains("divert") {
                    DiagnosticItem {
                        id: "windivert".into(),
                        title: "Драйвер WinDivert загружен".into(),
                        outcome: Outcome::Ok,
                        detail: "driverquery обнаружил Divert-драйвер.".into(),
                        hint: None,
                    }
                } else {
                    DiagnosticItem {
                        id: "windivert".into(),
                        title: "WinDivert не найден".into(),
                        outcome: Outcome::Fail,
                        detail: "Драйвер WinDivert отсутствует в системе.".into(),
                        hint: Some("Запустите winws.exe — он установит драйвер автоматически.".into()),
                    }
                }
            }
            Err(_) => DiagnosticItem {
                id: "windivert".into(),
                title: "Не удалось опросить драйверы".into(),
                outcome: Outcome::Warn,
                detail: "driverquery завершился с ошибкой.".into(),
                hint: None,
            },
        }
    }
    #[cfg(not(windows))]
    {
        DiagnosticItem {
            id: "windivert".into(),
            title: "WinDivert (Windows-only)".into(),
            outcome: Outcome::Info,
            detail: "Проверка доступна только на Windows.".into(),
            hint: None,
        }
    }
}

fn check_app_path_no_cyrillic() -> DiagnosticItem {
    let p = config::app_dir();
    let s = p.to_string_lossy();
    let has_cyr = s.chars().any(|c| (c as u32) >= 0x0400 && (c as u32) <= 0x04FF);
    if has_cyr {
        DiagnosticItem {
            id: "path-cyrillic".into(),
            title: "Путь содержит кириллицу".into(),
            outcome: Outcome::Fail,
            detail: format!("Путь {} содержит кириллические символы.", s),
            hint: Some(
                "zapret плохо работает с кириллицей в путях. Перенесите приложение в латиницу."
                    .into(),
            ),
        }
    } else {
        DiagnosticItem {
            id: "path-cyrillic".into(),
            title: "Путь без кириллицы".into(),
            outcome: Outcome::Ok,
            detail: format!("Путь: {}", s),
            hint: None,
        }
    }
}

fn check_secure_dns_hint() -> DiagnosticItem {
    DiagnosticItem {
        id: "secure-dns".into(),
        title: "Secure DNS (DoH)".into(),
        outcome: Outcome::Info,
        detail: "Включите Secure DNS в Chrome / Firefox / Windows 11 для корректной работы обхода."
            .into(),
        hint: Some(
            "Chrome → Settings → Privacy → Use secure DNS. Windows 11 → Settings → Network → DNS over HTTPS."
                .into(),
        ),
    }
}

fn check_hosts_file() -> DiagnosticItem {
    #[cfg(windows)]
    {
        let p = std::path::Path::new(r"C:\Windows\System32\drivers\etc\hosts");
        match std::fs::read_to_string(p) {
            Ok(s) => {
                let discord_ok = s.contains("gateway.discord.gg");
                if discord_ok {
                    DiagnosticItem {
                        id: "hosts".into(),
                        title: "hosts файл настроен".into(),
                        outcome: Outcome::Ok,
                        detail: "Обнаружены записи для Discord/Telegram.".into(),
                        hint: None,
                    }
                } else {
                    DiagnosticItem {
                        id: "hosts".into(),
                        title: "hosts файл требует обновления".into(),
                        outcome: Outcome::Warn,
                        detail: "Не найдены записи для gateway.discord.gg.".into(),
                        hint: Some("Нажмите «Update hosts file» на вкладке Updates.".into()),
                    }
                }
            }
            Err(_) => DiagnosticItem {
                id: "hosts".into(),
                title: "hosts файл недоступен".into(),
                outcome: Outcome::Warn,
                detail: "Не удалось прочитать hosts.".into(),
                hint: None,
            },
        }
    }
    #[cfg(not(windows))]
    {
        DiagnosticItem {
            id: "hosts".into(),
            title: "hosts (Windows-only)".into(),
            outcome: Outcome::Info,
            detail: "Проверка доступна только на Windows.".into(),
            hint: None,
        }
    }
}

fn check_discord_cache_present() -> DiagnosticItem {
    #[cfg(windows)]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let cache = std::path::Path::new(&appdata)
            .join("discord")
            .join("Cache");
        if cache.exists() {
            DiagnosticItem {
                id: "discord-cache".into(),
                title: "Кэш Discord найден".into(),
                outcome: Outcome::Info,
                detail: format!("{}", cache.display()),
                hint: Some(
                    "Если Discord внезапно перестал работать — очистите кэш кнопкой на вкладке Diagnostics."
                        .into(),
                ),
            }
        } else {
            DiagnosticItem {
                id: "discord-cache".into(),
                title: "Кэш Discord не найден".into(),
                outcome: Outcome::Info,
                detail: "Discord, по-видимому, не установлен.".into(),
                hint: None,
            }
        }
    }
    #[cfg(not(windows))]
    {
        DiagnosticItem {
            id: "discord-cache".into(),
            title: "Discord cache (Windows-only)".into(),
            outcome: Outcome::Info,
            detail: "Проверка доступна только на Windows.".into(),
            hint: None,
        }
    }
}

fn check_adhoc_process() -> DiagnosticItem {
    let snap = process::snapshot();
    if snap.running {
        DiagnosticItem {
            id: "adhoc-process".into(),
            title: "Фоновый процесс winws работает".into(),
            outcome: Outcome::Ok,
            detail: format!("PID {}", snap.pid.unwrap_or(0)),
            hint: None,
        }
    } else {
        DiagnosticItem {
            id: "adhoc-process".into(),
            title: "Фоновый процесс не запущен".into(),
            outcome: Outcome::Info,
            detail: "Запущенный вручную winws отсутствует.".into(),
            hint: None,
        }
    }
}

/// Clear the Discord cache (best effort, recursive rmdir of Cache/Cache_Data).
pub fn clear_discord_cache() -> Result<u64, String> {
    #[cfg(windows)]
    {
        let appdata = std::env::var("APPDATA").map_err(|e| e.to_string())?;
        let base = std::path::Path::new(&appdata)
            .join("discord")
            .join("Cache");
        if !base.exists() {
            return Ok(0);
        }
        let mut removed = 0u64;
        walk_remove(&base, &mut removed).map_err(|e| e.to_string())?;
        Ok(removed)
    }
    #[cfg(not(windows))]
    {
        let _ = clear_discord_cache;
        Ok(0)
    }
}

#[cfg(windows)]
fn walk_remove(p: &std::path::Path, removed: &mut u64) -> std::io::Result<()> {
    if p.is_dir() {
        for entry in std::fs::read_dir(p)? {
            let entry = entry?;
            walk_remove(&entry.path(), removed)?;
        }
        let _ = std::fs::remove_dir(p);
    } else {
        if std::fs::remove_file(p).is_ok() {
            *removed += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_runs_without_panic() {
        let r = run_diagnostics();
        assert!(!r.items.is_empty());
        let total = r.passed + r.warned + r.failed;
        // info items are not counted in those three
        assert!(total <= r.items.len());
    }

    #[test]
    fn outcomes_serialise_lowercase() {
        let s = serde_json::to_string(&Outcome::Ok).unwrap();
        assert_eq!(s, "\"ok\"");
        let s = serde_json::to_string(&Outcome::Fail).unwrap();
        assert_eq!(s, "\"fail\"");
    }

    #[test]
    fn winws_check_is_fail_in_test_env() {
        // winws.exe is not bundled in the test env, so this should fail
        let i = check_winws_present();
        assert_eq!(i.outcome, Outcome::Fail);
    }
}
