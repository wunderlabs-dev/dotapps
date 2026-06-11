//! Data directory integrity checks.
//!
//! Validates `~/.opnble/` structure before platform-specific setup runs.

use std::fs;
use std::path::Path;

use crate::constants::paths;
use crate::error::AppError;

/// Call during app startup, before platform-specific setup runs.
///
/// Creates `~/.opnble/{repos,vm,logs}/` if missing and validates the state
/// file when present.
#[tauri::command]
#[specta::specta]
pub async fn check_data_integrity() -> Result<(), AppError> {
    create_dir(&paths::base_dir()?)?;
    create_dir(&paths::repos_dir()?)?;
    create_dir(&paths::vm_dir()?)?;
    create_dir(&paths::logs_dir()?)?;

    let state = paths::state_file()?;
    if state.exists() {
        validate_state_file(&state)?;
    }

    Ok(())
}

// Manual `map_err` instead of `?` because `From<io::Error>` loses the path context.
fn create_dir(path: &Path) -> Result<(), AppError> {
    fs::create_dir_all(path).map_err(|e| AppError::StorageFailed {
        reason: format!("cannot create directory {}: {e}", path.display()),
    })
}

fn validate_state_file(path: &Path) -> Result<(), AppError> {
    let content = fs::read_to_string(path).map_err(|e| AppError::StorageFailed {
        reason: format!("cannot read state file {}: {e}", path.display()),
    })?;
    serde_json::from_str::<serde_json::Value>(&content).map_err(|e| AppError::StorageFailed {
        reason: format!("corrupt state file {}: {e}", path.display()),
    })?;
    Ok(())
}
