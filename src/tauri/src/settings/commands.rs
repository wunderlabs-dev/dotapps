//! Settings Tauri commands
//!
//! These commands use the new layered architecture with injected dependencies.

use std::sync::Arc;

use tauri::State;

use crate::error::AppError;
use crate::settings::{store::SettingsStore, types::AppSettings};

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn settings(store: State<Arc<dyn SettingsStore>>) -> Result<AppSettings, AppError> {
    store.get()
}

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn save_settings(
    store: State<Arc<dyn SettingsStore>>,
    settings: AppSettings,
) -> Result<(), AppError> {
    store.save(settings)
}
