//! Settings feature module
//!
//! Manages application settings with persistence.

pub mod commands;
pub mod store;
pub mod types;

pub use store::{JsonSettingsStore, SettingsStore};
pub use types::AppSettings;
