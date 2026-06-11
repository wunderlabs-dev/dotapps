//! Tauri IPC surface for diagnostics + version.

use tauri::AppHandle;

use crate::constants::paths;
use crate::error::AppError;

use super::export::{export, DiagnosticsExport};

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri commands receive owned values via IPC deserialization"
)]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
#[specta::specta]
pub async fn export_diagnostics(app: AppHandle) -> Result<DiagnosticsExport, AppError> {
    let version = app.package_info().version.to_string();
    tokio::task::spawn_blocking(move || export(&version))
        .await
        .map_err(|e| AppError::StorageFailed {
            reason: format!("diagnostics export task failed: {e}"),
        })?
}

#[tauri::command]
#[specta::specta]
pub async fn reveal_logs_folder() -> Result<String, AppError> {
    let path = paths::logs_dir()?;
    std::fs::create_dir_all(&path).map_err(|e| AppError::StorageFailed {
        reason: format!("cannot create logs directory {}: {e}", path.display()),
    })?;
    Ok(path.to_string_lossy().into_owned())
}
