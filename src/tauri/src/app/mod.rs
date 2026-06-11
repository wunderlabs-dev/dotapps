//! Tauri app glue: bootstrap, command registration, and shutdown.
//!
//! `main.rs` calls into this via the `run` and `graceful_shutdown`
//! re-exports. Domain logic lives in `crate::projects`, `crate::vm`, and
//! `crate::auth`; this module is wiring, not behavior.

pub mod bootstrap;
pub mod integrity;
pub mod logging;
pub mod panic_hook;
pub mod platform;
pub mod register;
pub mod shutdown;
pub mod updater;
pub mod window_commands;

pub use bootstrap::run;
pub use shutdown::graceful_shutdown;
