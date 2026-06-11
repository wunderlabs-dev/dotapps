//! `VmRuntime` - Runtime implementation using `VmGateway`
//!
//! Runs containers inside the VM via the ttrpc agent.
//! Repos are shared via `VirtioFS` (no SFTP sync needed).
//! File watchers trigger touch RPCs for hot reload.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use super::file_watcher::FileWatcher;
use super::VmGateway;
use crate::constants;
use crate::error::AppError;
use crate::infrastructure::runtime::Runtime;

/// Runtime implementation that runs containers inside a VM.
///
/// On macOS, this wraps `VmGateway` which uses either `VfkitVM` (Apple Silicon)
/// or `QemuVM` (Intel) to run a Linux VM with Podman inside.
/// File watchers per project trigger touch RPCs for hot reload.
pub struct VmRuntime {
    manager: Arc<Mutex<Option<VmGateway>>>,
    watchers: Mutex<HashMap<String, FileWatcher>>,
    port_forwards: Mutex<HashMap<String, JoinHandle<()>>>,
}

impl VmRuntime {
    /// Create a new `VmRuntime` with a shared `VmGateway` reference.
    ///
    /// The gateway may be None if the VM hasn't been initialized yet.
    /// Operations will fail with an appropriate error until the VM is running.
    pub fn new(manager: Arc<Mutex<Option<VmGateway>>>) -> Self {
        Self {
            manager,
            watchers: Mutex::new(HashMap::new()),
            port_forwards: Mutex::new(HashMap::new()),
        }
    }

    /// Get a reference to the inner `VmGateway`, or return an error if not running.
    fn vm_not_running_error() -> AppError {
        AppError::VmNotRunning
    }

    fn validate_repo_path(repo_path: &Path) -> Result<(), AppError> {
        if !repo_path.exists() {
            return Err(AppError::ContainerFailed {
                reason: format!("cannot find repository path: {}", repo_path.display()),
            });
        }
        if !repo_path.is_dir() {
            return Err(AppError::ContainerFailed {
                reason: format!(
                    "cannot use repository path (not a directory): {}",
                    repo_path.display()
                ),
            });
        }
        Ok(())
    }
}

#[async_trait]
impl Runtime for VmRuntime {
    async fn start(
        &self,
        project_id: &str,
        repo_path: &Path,
        host_port: u16,
    ) -> Result<(), AppError> {
        Self::validate_repo_path(repo_path)?;

        // Clone the agent Arc while briefly holding the lock, then release
        // so the RPC doesn't hold the manager mutex for the entire call.
        let agent = {
            let guard = self.manager.lock().await;
            let manager = guard.as_ref().ok_or_else(Self::vm_not_running_error)?;
            Arc::clone(manager.agent())
        };

        // Guest-side path where the project is mounted via VirtioFS
        let guest_project_path = format!("{}/{project_id}", constants::vm::VIRTIOFS_GUEST_MOUNT);

        // Best-effort readiness checks for shared mount and project path.
        // Gate aligned with file_watcher: OPNBLE_FS_OPS_V2 must be enabled (not "0")
        // and agent must support fs ops v2.
        let fs_ops_v2_gate = std::env::var("OPNBLE_FS_OPS_V2").is_ok_and(|v| v != "0");
        if fs_ops_v2_gate && agent.fs_ops_v2_supported().await.unwrap_or(false) {
            agent
                .ensure_path(constants::vm::VIRTIOFS_GUEST_MOUNT, true)
                .await
                .map_err(|e| AppError::ContainerFailed {
                    reason: format!("cannot verify vm mount readiness: {e}"),
                })?;
            agent
                .ensure_path(&guest_project_path, true)
                .await
                .map_err(|e| AppError::ContainerFailed {
                    reason: format!("cannot verify vm project path readiness: {e}"),
                })?;
        }

        // start_container returns the docker container hash, but we use
        // project_id as the canonical identifier because the agent derives
        // container names from it (e.g., "opnble-{project_id}").
        // With host networking (set in container.rs), port bindings are ignored
        // and the dev server binds directly to the VM's network. Pass host_port
        // so the server listens on the same port the vsock forwarder targets.
        let (_container_hash, _actual_port) = agent
            .start_container(&super::agent_client::StartContainerParams {
                project_id,
                repo_path: &guest_project_path,
                port: host_port,
                image: crate::constants::containers::NODE_IMAGE,
                cmd: &crate::constants::containers::dev_cmd(host_port),
                internal_port: host_port,
            })
            .await
            .map_err(|e| AppError::ContainerFailed {
                reason: e.to_string(),
            })?;

        // Expose container port on host via vsock port forwarding
        let vsock_path = {
            let guard = self.manager.lock().await;
            guard.as_ref().map(|m| m.vsock_socket_path().to_path_buf())
        };
        if let Some(vsock_path) = vsock_path {
            // With host networking the dev server listens directly on host_port
            // inside the VM. The vsock forwarder maps host:host_port -> VM:host_port.
            let handle = super::port_forward::start_forwarding(&vsock_path, host_port, host_port);
            self.port_forwards
                .lock()
                .await
                .insert(project_id.to_string(), handle);
        } else {
            tracing::warn!("cannot start port forwarding: vsock socket not available");
        }

        // Start file watcher for hot reload
        match FileWatcher::start(repo_path, guest_project_path, agent) {
            Ok(watcher) => {
                self.watchers
                    .lock()
                    .await
                    .insert(project_id.to_string(), watcher);
            }
            Err(e) => {
                // Non-fatal: container runs fine, just no hot reload via touch
                tracing::warn!("cannot start file watcher for {project_id}: {e}");
            }
        }

        Ok(())
    }

    async fn stop(&self, container_id: &str) -> Result<(), AppError> {
        // Stop port forwarding for this project
        if let Some(handle) = self.port_forwards.lock().await.remove(container_id) {
            super::port_forward::stop_forwarding(&handle);
        }

        // Stop and remove the file watcher for this project
        if let Some(watcher) = self.watchers.lock().await.remove(container_id) {
            watcher.stop().await;
        }

        // Clone the agent while briefly holding the lock, then release
        // so the stop RPC doesn't hold the manager mutex.
        let agent = {
            let guard = self.manager.lock().await;
            let manager = guard.as_ref().ok_or_else(Self::vm_not_running_error)?;
            Arc::clone(manager.agent())
        };

        // In VmRuntime, container_id is actually project_id
        // The VmGateway uses project_id to derive the actual container name
        agent
            .stop_container(container_id)
            .await
            .map_err(|e| AppError::ContainerFailed {
                reason: e.to_string(),
            })
    }

    async fn remove(&self, container_id: &str) -> Result<(), AppError> {
        let agent = {
            let guard = self.manager.lock().await;
            let manager = guard.as_ref().ok_or_else(Self::vm_not_running_error)?;
            Arc::clone(manager.agent())
        };

        agent
            .remove_container(container_id)
            .await
            .map_err(|e| AppError::ContainerFailed {
                reason: e.to_string(),
            })
    }

    async fn is_running(&self, container_id: &str) -> bool {
        // Clone the agent while briefly holding the lock, then release
        // so the status RPC doesn't hold the manager mutex.
        let agent = {
            let guard = self.manager.lock().await;
            match guard.as_ref() {
                Some(manager) => Arc::clone(manager.agent()),
                None => return false,
            }
        };
        agent
            .container_status(container_id)
            .await
            .is_ok_and(|info| info.status == "running")
    }

    async fn is_reachable(&self) -> bool {
        self.manager.lock().await.is_some()
    }

    async fn run_to_completion(
        &self,
        build_id: &str,
        repo_path: &Path,
        cmd: &str,
    ) -> Result<crate::infrastructure::runtime::BuildResult, AppError> {
        Self::validate_repo_path(repo_path)?;

        let agent = {
            let guard = self.manager.lock().await;
            let manager = guard.as_ref().ok_or_else(Self::vm_not_running_error)?;
            Arc::clone(manager.agent())
        };

        // Derive the guest-side repo path from the original project's repo_path.
        // repo_path on host is ~/.opnble/repos/<project_id>, the project_id is the
        // last path component. The guest mount is /repos/<project_id>.
        let project_dir = repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| AppError::ContainerFailed {
                reason: format!("cannot derive project dir from {}", repo_path.display()),
            })?;
        let guest_project_path = format!("{}/{project_dir}", constants::vm::VIRTIOFS_GUEST_MOUNT);

        // Start a temporary build container (separate from the dev container)
        let _result = agent
            .start_container(&super::agent_client::StartContainerParams {
                project_id: build_id,
                repo_path: &guest_project_path,
                port: 0,
                image: crate::constants::containers::NODE_IMAGE,
                cmd,
                internal_port: 0,
            })
            .await
            .map_err(|e| AppError::ContainerFailed {
                reason: format!("cannot start build container: {e}"),
            })?;

        // Poll container status until it exits (max 10 minutes).
        // Stream recent log lines on each poll for real-time visibility.
        let poll_interval = std::time::Duration::from_secs(3);
        let max_wait = std::time::Duration::from_mins(10);
        let start_time = std::time::Instant::now();
        loop {
            tokio::time::sleep(poll_interval).await;

            if start_time.elapsed() > max_wait {
                let _ = agent.stop_container(build_id).await;
                let _ = agent.remove_container(build_id).await;
                return Err(crate::publish::PublishError::BuildFailed {
                    tail: "build timed out after 10 minutes".into(),
                }
                .into());
            }

            // Show the latest container log line for real-time visibility.
            if let Ok(lines) = agent.stream_logs(build_id, 1).await {
                if let Some((last_line, _)) = lines.last() {
                    let elapsed = start_time.elapsed().as_secs();
                    tracing::debug!(
                        build_id = %build_id,
                        elapsed_s = elapsed,
                        last_line = %last_line,
                        "container log line"
                    );
                }
            }

            let status = agent.container_status(build_id).await;
            let still_running = status.as_ref().is_ok_and(|info| info.status == "running");
            if !still_running {
                let elapsed = start_time.elapsed().as_secs();
                tracing::info!(
                    build_id = %build_id,
                    elapsed_s = elapsed,
                    "run_to_completion exited"
                );
                break;
            }
        }

        // Capture logs
        let logs = agent
            .stream_logs(build_id, 200)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(line, _stream)| line)
            .collect::<Vec<_>>()
            .join("\n");

        // Get exit code via project health (has exit_code field)
        let exit_code = agent
            .project_health(build_id, 0)
            .await
            .map_or(-1, |h| h.exit_code);

        // Clean up
        let _ = agent.remove_container(build_id).await;

        Ok(crate::infrastructure::runtime::BuildResult { exit_code, logs })
    }

    async fn tail_logs(&self, project_id: &str, lines: u32) -> Result<Vec<String>, AppError> {
        let agent = {
            let guard = self.manager.lock().await;
            let manager = guard.as_ref().ok_or_else(Self::vm_not_running_error)?;
            Arc::clone(manager.agent())
        };

        let entries = agent.stream_logs(project_id, lines).await?;
        Ok(entries.into_iter().map(|(line, _stream)| line).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_vm_runtime_creation() {
        // VmRuntime can be created with None gateway
        let manager: Arc<Mutex<Option<VmGateway>>> = Arc::new(Mutex::new(None));
        let _runtime = VmRuntime::new(manager);
    }

    #[tokio::test]
    async fn test_vm_not_running_returns_error() {
        let manager: Arc<Mutex<Option<VmGateway>>> = Arc::new(Mutex::new(None));
        let runtime = VmRuntime::new(manager);

        // Operations should fail when VM is not running
        let result = runtime.stop("test-project").await;
        assert!(result.is_err());

        let is_running = runtime.is_running("test-project").await;
        assert!(!is_running);
    }

    #[tokio::test]
    async fn test_start_without_manager_returns_error() {
        let manager: Arc<Mutex<Option<VmGateway>>> = Arc::new(Mutex::new(None));
        let runtime = VmRuntime::new(manager);
        let repo = std::env::temp_dir().join("opnble-vm-runtime-test");
        let _ = fs::create_dir_all(&repo);
        let result = runtime.start("test-project", &repo, 3001).await;
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn test_start_fails_when_repo_missing() {
        let manager: Arc<Mutex<Option<VmGateway>>> = Arc::new(Mutex::new(None));
        let runtime = VmRuntime::new(manager);
        let missing = std::env::temp_dir().join("opnble-vm-runtime-missing");
        let result = runtime.start("test-project", &missing, 3001).await;
        assert!(result.is_err());
        let message = result.err().unwrap().to_string();
        assert!(message.contains("cannot find repository path"));
    }
}
