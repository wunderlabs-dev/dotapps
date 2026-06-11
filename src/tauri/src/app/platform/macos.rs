use std::sync::Arc;

use tauri::{AppHandle, Builder, CloseRequestApi, Manager, State, Window, Wry};

use super::PlatformBootstrap;
use crate::auth::Authenticator;
use crate::error::AppError;
use crate::infrastructure::Runtime;
use crate::mcp::exec::{AgentExecRunner, ExecRunner};
use crate::projects::ProjectStore;
use crate::settings::SettingsStore;
use crate::tray;
use crate::tunnel::TunnelCoordinator;
use crate::vm::{
    ContainerStatus, ResourceStats, VmImageDownloader, VmImageStatus, VmLifecycle, VmRuntime,
};

/// Use on macOS to bootstrap the VM-based container runtime.
pub struct MacosBoot {
    vm_lifecycle: Arc<VmLifecycle>,
    image_downloader: Arc<VmImageDownloader>,
}

impl MacosBoot {
    pub fn new() -> Self {
        Self {
            vm_lifecycle: Arc::new(VmLifecycle::new()),
            image_downloader: Arc::new(VmImageDownloader::new()),
        }
    }
}

impl PlatformBootstrap for MacosBoot {
    fn runtime(&self) -> Arc<dyn Runtime> {
        Arc::new(VmRuntime::new(self.vm_lifecycle.manager()))
    }

    fn manage_state(&self, builder: Builder<Wry>) -> Builder<Wry> {
        builder
            .manage(Arc::clone(&self.vm_lifecycle))
            .manage(Arc::clone(&self.image_downloader))
    }

    fn exec_runner(&self) -> Arc<dyn ExecRunner> {
        Arc::new(AgentExecRunner::new(self.vm_lifecycle.manager()))
    }

    fn on_setup(&self, app: &AppHandle) -> Result<(), AppError> {
        let settings_store = app.state::<Arc<dyn SettingsStore>>();
        let auto_start_vm = settings_store.get().map_or(true, |s| s.auto_start_vm);

        let lifecycle = app.state::<Arc<VmLifecycle>>();
        let lifecycle_clone = Arc::clone(lifecycle.inner());
        let app_handle = app.clone();
        let tunnel = Arc::clone(app.state::<Arc<TunnelCoordinator>>().inner());
        let store = Arc::clone(app.state::<Arc<dyn ProjectStore>>().inner());
        let auth = Arc::clone(app.state::<Arc<Authenticator>>().inner());

        tauri::async_runtime::spawn(async move {
            if !auto_start_vm {
                tracing::info!("Auto-start disabled in settings");
                return;
            }

            tray::update_tray_status(&app_handle, tray::STATUS_STARTING);

            match lifecycle_clone.start(&app_handle).await {
                Ok(()) => {
                    tray::update_tray_status(&app_handle, tray::STATUS_RUNNING);
                    tracing::info!("VM started successfully");

                    if let Ok(Some(token)) = auth.valid_token("github").await {
                        tunnel
                            .cleanup_orphaned_tunnels(&token, store.as_ref())
                            .await;
                    }
                }
                Err(e) => {
                    tray::update_tray_status(&app_handle, tray::STATUS_STOPPED);
                    tracing::error!("cannot start VM: {e}");
                }
            }
        });

        Ok(())
    }

    fn on_close_requested(&self, window: &Window, api: &CloseRequestApi) {
        let _ = window.hide();
        api.prevent_close();

        let any_visible = window
            .app_handle()
            .webview_windows()
            .values()
            .any(|w| w.is_visible().unwrap_or(false));
        if !any_visible {
            tray::hide_dock_icon();
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn stop_vm(
    app: tauri::AppHandle,
    lifecycle: State<'_, Arc<VmLifecycle>>,
) -> Result<(), AppError> {
    crate::app::shutdown::stop_all_projects(&app).await;

    if !lifecycle.is_running().await {
        crate::tray::update_tray_icon(&app, false);
        return Ok(());
    }

    lifecycle.stop().await?;

    crate::tray::update_tray_icon(&app, false);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn init_vm(
    app: tauri::AppHandle,
    lifecycle: State<'_, Arc<VmLifecycle>>,
) -> Result<(), AppError> {
    if lifecycle.is_running().await {
        tracing::info!("VM already running, skipping init");
        return Ok(());
    }

    Ok(lifecycle.start(&app).await?)
}

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned AppHandle by convention"
)]
pub fn is_vm_running(app: tauri::AppHandle) -> bool {
    crate::tray::is_vm_running(&app)
}

#[tauri::command]
#[specta::specta]
pub async fn vm_status(lifecycle: State<'_, Arc<VmLifecycle>>) -> Result<String, AppError> {
    if lifecycle.is_running().await {
        Ok("running".to_string())
    } else {
        Ok("stopped".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub async fn container_status(
    lifecycle: State<'_, Arc<VmLifecycle>>,
    project_id: String,
) -> Result<ContainerStatus, AppError> {
    Ok(lifecycle.container_status(&project_id).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_running_containers(
    lifecycle: State<'_, Arc<VmLifecycle>>,
) -> Result<Vec<ContainerStatus>, AppError> {
    Ok(lifecycle.list_containers().await?)
}

#[tauri::command]
#[specta::specta]
pub async fn vm_stats(lifecycle: State<'_, Arc<VmLifecycle>>) -> Result<ResourceStats, AppError> {
    Ok(lifecycle.resource_stats().await?)
}

#[tauri::command]
#[specta::specta]
pub async fn vm_exec(
    lifecycle: State<'_, Arc<VmLifecycle>>,
    command: String,
) -> Result<String, AppError> {
    let (exit_code, logs) = lifecycle.exec(&command).await?;
    Ok(format!("exit_code={exit_code}\n{logs}"))
}

// Stubs for commands that only exist on other platforms.
// Needed so the specta builder can register a unified command set
// and generate TypeScript bindings for all platforms from macOS.

#[tauri::command]
#[specta::specta]
pub fn check_setup_status() -> bool {
    true
}

#[tauri::command]
#[specta::specta]
pub async fn run_initial_setup() -> Result<(), AppError> {
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn check_windows_setup() -> String {
    "not_available".to_string()
}

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::unnecessary_wraps,
    reason = "signature must match Windows command for consistent specta bindings"
)]
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
pub async fn vm_image_status(
    downloader: State<'_, Arc<VmImageDownloader>>,
) -> Result<VmImageStatus, AppError> {
    downloader.status().await
}

#[tauri::command]
#[specta::specta]
pub async fn download_vm_image(
    app: AppHandle,
    downloader: State<'_, Arc<VmImageDownloader>>,
) -> Result<(), AppError> {
    Arc::clone(downloader.inner()).download(app).await
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_vm_image_download(
    downloader: State<'_, Arc<VmImageDownloader>>,
) -> Result<(), AppError> {
    downloader.cancel().await;
    Ok(())
}
