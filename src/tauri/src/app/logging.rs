//! File-based tracing initialization.
//!
//! GUI-launched apps eat stderr, so beta testers cannot send useful logs
//! without a file sink. This module installs a daily-rotated file appender
//! alongside the stderr layer that already existed.

use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, registry};

use crate::constants::paths;
use crate::error::AppError;

/// Initialize tracing. The returned guard must live for the app's lifetime;
/// dropping it shuts down the background writer thread and logs are lost.
pub fn init() -> Result<WorkerGuard, AppError> {
    let logs_dir = paths::logs_dir()?;
    std::fs::create_dir_all(&logs_dir).map_err(|e| AppError::StorageFailed {
        reason: format!("cannot create logs directory {}: {e}", logs_dir.display()),
    })?;

    let env_filter = default_filter();

    let file_appender = tracing_appender::rolling::daily(&logs_dir, "dotapps.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(std::io::stderr).with_ansi(true))
        .with(
            fmt::layer()
                .with_writer(file_writer)
                .with_ansi(false)
                .with_target(true),
        )
        .init();

    tracing::info!(
        log_dir = %logs_dir.display(),
        "tracing initialized with file appender"
    );

    Ok(guard)
}

/// `RUST_LOG=...` overrides the default. Production defaults to `info` for
/// the host crate so a tester's diagnostics zip is useful without leaking
/// debug-only data.
fn default_filter() -> EnvFilter {
    let default = if cfg!(debug_assertions) {
        "opnble_lib=debug"
    } else {
        "opnble_lib=info"
    };
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default))
}

/// Sibling file that the panic hook writes to when tracing isn't fully up
/// yet or has been torn down. Kept separate from the rolling app log so
/// crash-time triage doesn't have to grep across daily files.
pub fn panic_log_path() -> Result<PathBuf, AppError> {
    paths::logs_dir().map(|d| d.join("panic.log"))
}
