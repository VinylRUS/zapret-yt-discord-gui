//! Crash diagnostics.
//!
//! Tauri release builds used to set `panic = "abort"`, which made every
//! panic — in the main thread or in a background tokio task — kill the
//! process instantly with no dialog and no log. Users saw the tray icon
//! flash and disappear, with nothing to diagnose from.
//!
//! `Cargo.toml` now uses `panic = "unwind"` and this module installs a
//! panic hook that:
//!   1. writes a full `panic.log` into the app logs dir,
//!   2. prints the panic to stderr (visible if launched from a console),
//!   3. pops a native Windows MessageBox so the user immediately sees the
//!      crash reason instead of a silent exit.
//!
//! Call [`install_panic_hook`] once at the very top of `run()`.

use crate::config;
use std::backtrace::Backtrace;

/// Wide-string helper for the Win32 message box.
fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Install the crash-reporting panic hook. Idempotent-ish: it chains the
/// previous hook so default behaviour (printing to stderr) still happens.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".into());
        let payload = info
            .payload()
            .downcast_ref::<&'static str>()
            .map(|s| (*s).to_string())
            .or_else(|| {
                info.payload()
                    .downcast_ref::<String>()
                    .map(|s| s.clone())
            })
            .unwrap_or_else(|| "<non-string panic payload>".to_string());

        let bt = Backtrace::force_capture();
        let report = format!(
            "Zapret GUI panic\n\
             timestamp : {}\n\
             location  : {}\n\
             payload   : {}\n\
             thread    : {:?}\n\
             \n\
             --- backtrace ---\n{}\n",
            chrono::Utc::now().to_rfc3339(),
            location,
            payload,
            std::thread::current().name().unwrap_or("<main>"),
            bt
        );

        // 1. log file
        let log_dir = config::logs_dir();
        let _ = std::fs::create_dir_all(&log_dir);
        let _ = std::fs::write(log_dir.join("panic.log"), &report);

        // 2. stderr (visible when launched from cmd / powershell)
        eprintln!("{report}");

        // 3. native message box on Windows so the crash is never silent
        #[cfg(windows)]
        {
            use windows::core::PCWSTR;
            use windows::Win32::UI::WindowsAndMessaging::{
                MessageBoxW, MB_ICONERROR, MB_OK,
            };
            let body = encode_wide(&format!(
                "Zapret GUI crashed and will close.\n\n\
                 Reason: {payload}\n\
                 Location: {location}\n\n\
                 A crash log was written to:\n{}\n\n\
                 Please send that file to the developer.",
                log_dir.join("panic.log").display()
            ));
            let title = encode_wide("Zapret GUI — crash");
            // SAFETY: MessageBoxW with a null owner window is always safe to call.
            unsafe {
                let _ = MessageBoxW(
                    None,
                    PCWSTR::from_raw(body.as_ptr()),
                    PCWSTR::from_raw(title.as_ptr()),
                    MB_ICONERROR | MB_OK,
                );
            }
        }

        previous(info);
    }));
}
