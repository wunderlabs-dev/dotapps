//! Opnble Tauri Application
//!
//! Crate root intentionally stays thin. App composition and command registration
//! live under `app/`.

mod app;
pub mod apps;
mod auth;
mod constants;
mod diagnostics;
mod error;
pub mod infrastructure;
mod mcp;
mod projects;
mod publish;
mod settings;
mod snapshots;
#[cfg(any(
    all(target_os = "macos", target_arch = "x86_64"),
    target_os = "windows"
))]
mod ssh;
mod tray;
mod tunnel;

pub mod vm;

pub use app::graceful_shutdown;
pub use error::AppError;

/// Re-exported so the `regen_bindings` example can rebuild TypeScript bindings
/// without spinning up the Tauri runtime.
#[cfg(target_os = "macos")]
pub use app::register::{macos_specta_builder, typescript_export_config};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> Result<(), AppError> {
    app::run()
}
