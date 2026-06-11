//! Auto-update orchestration: periodic check, event emission, and the install
//! command. Runs as a background task spawned at startup.
//!
//! The `pubkey` in `tauri.conf.json` is a placeholder until the maintainer
//! generates a signing key: see `docs/release-setup.md`.

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

use crate::error::AppError;

const STARTUP_DELAY: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_hours(6);
const UPDATE_AVAILABLE_EVENT: &str = "update-available";
const UPDATE_PROGRESS_EVENT: &str = "update-progress";

#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "phase")]
#[expect(
    dead_code,
    reason = "Started and Finished variants are constructed on the frontend via the generated TS type"
)]
pub enum UpdateProgress {
    Started { content_length: Option<u64> },
    Downloading { downloaded: u64, total: Option<u64> },
    Finished,
    Installing,
}

/// Spawn the periodic check loop. Called once from bootstrap.
pub fn schedule_periodic_check(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_DELAY).await;
        loop {
            if let Err(e) = check_and_emit(&app).await {
                tracing::warn!("update check failed: {e}");
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
}

/// Check for an update and emit `update-available` if one exists. Does not
/// download: that's user-initiated.
async fn check_and_emit(app: &AppHandle) -> Result<(), AppError> {
    let updater = app.updater()?;
    let Some(update) = updater.check().await? else {
        return Ok(());
    };
    let info = UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone(),
        date: update.date.map(|d| d.to_string()),
    };
    app.emit(UPDATE_AVAILABLE_EVENT, &info)?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, AppError> {
    let updater = app.updater()?;
    let Some(update) = updater.check().await? else {
        return Ok(None);
    };
    Ok(Some(UpdateInfo {
        version: update.version,
        current_version: update.current_version,
        notes: update.body,
        date: update.date.map(|d| d.to_string()),
    }))
}

#[tauri::command]
#[specta::specta]
pub async fn install_update(app: AppHandle) -> Result<(), AppError> {
    download_and_restart(app).await
}

async fn download_and_restart(app: AppHandle) -> Result<(), AppError> {
    let updater = app.updater()?;
    let Some(update) = updater.check().await? else {
        return Err(AppError::Internal {
            reason: "no update available".into(),
        });
    };

    let app_for_progress = app.clone();
    update
        .download_and_install(
            move |chunk_len, total| {
                let payload = UpdateProgress::Downloading {
                    downloaded: u64::try_from(chunk_len).unwrap_or(0),
                    total,
                };
                if let Err(e) = app_for_progress.emit(UPDATE_PROGRESS_EVENT, &payload) {
                    tracing::warn!("cannot emit update-progress: {e}");
                }
            },
            move || {
                tracing::info!("update download complete; installing");
            },
        )
        .await?;

    app.emit(UPDATE_PROGRESS_EVENT, &UpdateProgress::Installing)?;
    app.restart()
}
