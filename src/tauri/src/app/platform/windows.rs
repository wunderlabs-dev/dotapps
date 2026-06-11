use std::sync::Arc;

use tauri::{Builder, Manager, State, Wry};
use tokio::sync::Mutex;

use super::PlatformBootstrap;
use crate::error::AppError;
use crate::infrastructure::{Runtime, StubRuntime};
use crate::vm::wsl::{WSL2Runtime, WSL2};
use crate::vm::wsl_setup::{check_wsl_status, enable_wsl};
use crate::vm::VmError;

/// Use on Windows to bootstrap the WSL2-based container runtime.
pub struct WindowsBoot {
    wsl_state: Arc<Mutex<Option<WSL2>>>,
}

impl WindowsBoot {
    pub fn new() -> Self {
        Self {
            wsl_state: Arc::new(Mutex::new(None)),
        }
    }
}

impl PlatformBootstrap for WindowsBoot {
    fn runtime(&self) -> Arc<dyn Runtime> {
        if WSL2::is_available() {
            tracing::info!("WSL2 detected, using WSL2Runtime");
            Arc::new(WSL2Runtime::new(Arc::clone(&self.wsl_state)))
        } else {
            tracing::warn!(
                "WSL2 not available; container operations disabled until setup completes"
            );
            Arc::new(StubRuntime)
        }
    }

    fn manage_state(&self, builder: Builder<Wry>) -> Builder<Wry> {
        builder.manage(Arc::clone(&self.wsl_state))
    }
}

#[tauri::command]
#[specta::specta]
pub fn check_windows_setup() -> String {
    check_wsl_status().to_string()
}

#[tauri::command]
#[specta::specta]
pub fn enable_wsl_windows() -> Result<bool, AppError> {
    Ok(enable_wsl()?)
}

#[tauri::command]
#[specta::specta]
pub async fn init_wsl(
    app: tauri::AppHandle,
    wsl: State<'_, Arc<Mutex<Option<WSL2>>>>,
) -> Result<(), AppError> {
    let mut wsl_guard = wsl.lock().await;

    if wsl_guard.is_some() {
        return Ok(());
    }

    if !WSL2::is_available() {
        return Err(VmError::SetupFailed {
            reason: "cannot initialize wsl2: not available".to_string(),
        }
        .into());
    }

    let mut wsl2 = WSL2::new();

    if !wsl2.is_distro_installed() {
        // Use Tauri's resource resolver (matches vfkit.rs pattern)
        let resource_dir = app
            .path()
            .resource_dir()
            .map_err(|e| VmError::ConfigInvalid {
                reason: format!("cannot resolve resource dir: {e}"),
            })?;
        let tar_path = resource_dir.join("resources/windows/alpine-wsl.tar");

        // Install directory goes in AppData (user-writable, persistent)
        let install_dir = dirs::data_dir()
            .ok_or_else(|| VmError::ConfigInvalid {
                reason: "cannot resolve data directory".to_string(),
            })?
            .join("opnble")
            .join("wsl-distro");

        if tar_path.exists() {
            wsl2.install_distro(&tar_path, &install_dir)?;
        } else {
            return Err(VmError::ImageNotFound { path: tar_path }.into());
        }
    }

    wsl2.run("podman --version")?;

    *wsl_guard = Some(wsl2);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn reboot_windows() -> Result<(), AppError> {
    use std::process::Command;

    Command::new("shutdown")
        .args([
            "/r",
            "/t",
            "5",
            "/c",
            "dotapps needs to restart Windows to complete WSL2 setup.",
        ])
        .spawn()
        .map_err(|e| AppError::VmSetupFailed {
            reason: format!("cannot initiate reboot: {e}"),
        })?;

    Ok(())
}

// Stubs for commands that only exist on other platforms.
// Needed so the specta builder can register a unified command set.

#[tauri::command]
#[specta::specta]
pub async fn init_vm() -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn is_vm_running() -> bool {
    false
}

#[tauri::command]
#[specta::specta]
pub async fn vm_status() -> Result<String, AppError> {
    Ok("not_available".to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn container_status(_project_id: String) -> Result<(), AppError> {
    Err(AppError::VmNotRunning)
}

#[tauri::command]
#[specta::specta]
pub async fn list_running_containers() -> Result<Vec<String>, AppError> {
    Ok(Vec::new())
}

#[tauri::command]
#[specta::specta]
pub async fn vm_stats() -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn check_setup_status() -> bool {
    false
}

#[tauri::command]
#[specta::specta]
pub async fn run_initial_setup() -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn vm_image_status() -> Result<crate::vm::VmImageStatus, AppError> {
    Ok(crate::vm::VmImageStatus {
        needed: false,
        installed_version: None,
        target_version: None,
        compressed_size_bytes: None,
        uncompressed_size_bytes: None,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn download_vm_image() -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_vm_image_download() -> Result<(), AppError> {
    Ok(())
}
