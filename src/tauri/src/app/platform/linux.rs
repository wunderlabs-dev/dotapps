use std::sync::Arc;

use tauri::{Builder, State, Wry};

use super::PlatformBootstrap;
use crate::error::AppError;
use crate::infrastructure::{PodmanRuntime, Runtime, StubRuntime};
use crate::projects::runner::RunningContainers;
use crate::projects::types::ProjectId;

/// Use on Linux to bootstrap the native Podman runtime.
pub struct LinuxBoot;

impl PlatformBootstrap for LinuxBoot {
    fn runtime(&self) -> Arc<dyn Runtime> {
        match PodmanRuntime::new() {
            Ok(r) => Arc::new(r),
            Err(e) => {
                tracing::warn!("container runtime not available: {e}");
                tracing::warn!("container operations will fail until runtime is available");
                Arc::new(StubRuntime)
            }
        }
    }

    fn manage_state(&self, builder: Builder<Wry>) -> Builder<Wry> {
        builder
    }
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
pub fn check_setup_status() -> bool {
    std::process::Command::new("podman")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[tauri::command]
#[specta::specta]
pub async fn run_initial_setup(runtime: State<'_, Arc<dyn Runtime>>) -> Result<(), AppError> {
    runtime.ensure_image().await
}

#[derive(serde::Serialize, specta::Type)]
pub struct LinuxContainerStatus {
    pub id: String,
    pub name: String,
    pub status: String,
    pub port: Option<u16>,
}

#[tauri::command]
#[specta::specta]
pub async fn container_status(
    running: State<'_, Arc<RunningContainers>>,
    project_id: ProjectId,
) -> Result<LinuxContainerStatus, AppError> {
    if let Some(container_id) = running.get(&project_id).await {
        return Ok(LinuxContainerStatus {
            id: container_id.clone(),
            name: crate::constants::containers::name(&project_id),
            status: "running".to_string(),
            port: None,
        });
    }
    Err(AppError::NotFound {
        entity: "container".into(),
        id: project_id.to_string(),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn list_running_containers(
    running: State<'_, Arc<RunningContainers>>,
) -> Result<Vec<LinuxContainerStatus>, AppError> {
    let containers = running.list().await;
    let result: Vec<LinuxContainerStatus> = containers
        .iter()
        .map(|(project_id, container_id)| LinuxContainerStatus {
            id: container_id.clone(),
            name: crate::constants::containers::name(&project_id),
            status: "running".to_string(),
            port: None,
        })
        .collect();
    Ok(result)
}

#[derive(serde::Serialize, specta::Type)]
pub struct LinuxResourceStats {
    pub cpu_percent: f64,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub disk_available_mb: u64,
    pub disk_total_mb: u64,
}

#[tauri::command]
#[specta::specta]
pub async fn vm_stats() -> Result<LinuxResourceStats, AppError> {
    Ok(LinuxResourceStats {
        cpu_percent: 0.0,
        memory_used_mb: 0,
        memory_total_mb: 0,
        disk_available_mb: 0,
        disk_total_mb: 0,
    })
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
pub fn check_windows_setup() -> String {
    "not_available".to_string()
}

#[tauri::command]
#[specta::specta]
pub fn enable_wsl_windows() -> Result<bool, AppError> {
    Ok(false)
}

#[tauri::command]
#[specta::specta]
pub async fn init_wsl() -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn reboot_windows() -> Result<(), AppError> {
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
