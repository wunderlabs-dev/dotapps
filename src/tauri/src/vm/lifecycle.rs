//! VM Lifecycle Management
//!
//! Handles automatic VM startup on app launch and graceful shutdown.

use std::sync::Arc;

use tauri::AppHandle;
use tokio::sync::{watch, Mutex};

use super::gateway::{ContainerStatus, ResourceStats};
use super::{VmError, VmGateway};

fn config_path(
    result: Result<std::path::PathBuf, crate::error::AppError>,
) -> Result<std::path::PathBuf, VmError> {
    result.map_err(|e| VmError::ConfigInvalid {
        reason: e.to_string(),
    })
}

pub struct VmLifecycle {
    manager: Arc<Mutex<Option<VmGateway>>>,
    status_tx: watch::Sender<VmStatus>,
    status_rx: watch::Receiver<VmStatus>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VmStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error(String),
}

impl VmLifecycle {
    pub fn new() -> Self {
        let (status_tx, status_rx) = watch::channel(VmStatus::Stopped);
        Self {
            manager: Arc::new(Mutex::new(None)),
            status_tx,
            status_rx,
        }
    }

    pub async fn start(&self, app: &AppHandle) -> Result<(), VmError> {
        // Prevent concurrent starts: if already starting or running, skip
        let current = self.status_rx.borrow().clone();
        match current {
            VmStatus::Running => {
                tracing::info!("VM already running, skipping start");
                return Ok(());
            }
            VmStatus::Starting => {
                tracing::info!("VM already starting, skipping duplicate start");
                return Ok(());
            }
            VmStatus::Stopped | VmStatus::Stopping | VmStatus::Error(_) => {}
        }

        let _ = self.status_tx.send(VmStatus::Starting);

        match self.start_inner(app).await {
            Ok(()) => {
                let _ = self.status_tx.send(VmStatus::Running);
                Ok(())
            }
            Err(e) => {
                let _ = self.status_tx.send(VmStatus::Error(e.to_string()));
                Err(e)
            }
        }
    }

    async fn start_inner(&self, app: &AppHandle) -> Result<(), VmError> {
        let base_image = config_path(crate::constants::paths::vm_base_image())?;
        let vm_image = config_path(crate::constants::paths::vm_image())?;
        let repos_path = config_path(crate::constants::paths::repos_dir())?;

        // Ensure directories exist
        let vm_dir = config_path(crate::constants::paths::vm_dir())?;
        std::fs::create_dir_all(&vm_dir)?;
        std::fs::create_dir_all(&repos_path)?;

        // Check if base image exists
        if !base_image.exists() {
            return Err(VmError::ImageNotFound { path: base_image });
        }

        // Create APFS CoW clone of base image for this runtime session.
        // This protects the base image from corruption if vfkit is killed mid-write.
        super::clone::clone_file_or_copy(&base_image, &vm_image)?;

        let manager = VmGateway::new(app, &vm_image, repos_path)?;
        manager.start().await?;

        *self.manager.lock().await = Some(manager);
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), VmError> {
        let _ = self.status_tx.send(VmStatus::Stopping);

        let manager = {
            let mut guard = self.manager.lock().await;
            guard.take()
        };

        if let Some(manager) = manager {
            manager.stop().await?;
        }

        let _ = self.status_tx.send(VmStatus::Stopped);
        Ok(())
    }

    pub async fn is_running(&self) -> bool {
        self.manager.lock().await.is_some()
    }

    pub fn manager(&self) -> Arc<Mutex<Option<VmGateway>>> {
        Arc::clone(&self.manager)
    }

    /// Get status of a specific container
    pub async fn container_status(&self, project_id: &str) -> Result<ContainerStatus, VmError> {
        let manager = {
            let guard = self.manager.lock().await;
            guard.as_ref().ok_or(VmError::NotRunning)?.clone()
        };
        manager.container_status(project_id).await
    }

    /// List all running containers
    pub async fn list_containers(&self) -> Result<Vec<ContainerStatus>, VmError> {
        let manager = {
            let guard = self.manager.lock().await;
            guard.as_ref().ok_or(VmError::NotRunning)?.clone()
        };
        manager.list_containers().await
    }

    /// Run a shell command in a one-shot debug container inside the VM.
    pub async fn exec(&self, cmd: &str) -> Result<(i32, String), VmError> {
        let manager = {
            let guard = self.manager.lock().await;
            guard.as_ref().ok_or(VmError::NotRunning)?.clone()
        };
        manager.exec(cmd).await
    }

    /// Run a shell command directly on the VM host via the agent's `ExecHost`
    /// RPC (not inside a container). Returns (`exit_code`, stdout, stderr).
    ///
    /// Used by vibox app commands to drive podman. The agent Arc is cloned
    /// while briefly holding the lock so the RPC (up to 120s for large
    /// `podman load`s) does not hold the manager mutex.
    pub async fn exec_host(
        &self,
        cmd: &str,
    ) -> Result<(i32, String, String), crate::error::AppError> {
        let agent = {
            let guard = self.manager.lock().await;
            let manager = guard
                .as_ref()
                .ok_or(crate::error::AppError::VmNotRunning)?;
            Arc::clone(manager.agent())
        };
        agent.exec_host(cmd).await
    }

    /// vsock socket path for port forwarding, or None when the VM is not
    /// running.
    pub async fn vsock_path(&self) -> Option<std::path::PathBuf> {
        let guard = self.manager.lock().await;
        guard.as_ref().map(|m| m.vsock_socket_path().to_path_buf())
    }

    /// Get VM resource statistics
    pub async fn resource_stats(&self) -> Result<ResourceStats, VmError> {
        let manager = {
            let guard = self.manager.lock().await;
            guard.as_ref().ok_or(VmError::NotRunning)?.clone()
        };
        manager.resource_stats().await
    }
}

impl Default for VmLifecycle {
    fn default() -> Self {
        Self::new()
    }
}
