//! Capture Rust panics for offline triage.
//!
//! Tracing usually carries panics, but GUI launches lose stderr and a panic
//! during early bootstrap can fire before the subscriber is installed. The
//! hook writes a separate `panic.log` and also calls `tracing::error!` so
//! the rolling app log gets the same record when it is up.

use std::backtrace::Backtrace;
use std::fs::OpenOptions;
use std::io::Write;
use std::panic::PanicHookInfo;
use std::time::{SystemTime, UNIX_EPOCH};

/// Install the hook. Idempotent: replaces whatever hook is currently set,
/// then chains to it so any earlier hook (Tauri's, the test harness) still
/// runs.
pub fn init() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info: &PanicHookInfo<'_>| {
        let backtrace = Backtrace::force_capture();
        write_to_panic_log(info, &backtrace);
        tracing::error!(panic = %info, backtrace = %backtrace, "application panic");
        default(info);
    }));
}

fn write_to_panic_log(info: &PanicHookInfo<'_>, backtrace: &Backtrace) {
    let Ok(path) = super::logging::panic_log_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let _ = writeln!(file, "--- panic ts={ts}\n{info}\nbacktrace:\n{backtrace}\n");
}
