//! VM Gateway: host-to-VM bridge for container operations
//!
//! Provides a unified interface for:
//! - Starting/stopping the VM
//! - Running containers via ttrpc agent
//! - Resource monitoring
//!
//! Uses `VfkitVM` for both Apple Silicon and Intel macOS hosts to preserve
//! vsock + `VirtioFS` compatibility required by ttrpc and gitfs semantics.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::AppHandle;
use tokio::sync::Mutex;

use super::agent_client::AgentClient;
use super::vfkit::VfkitVM;
use super::{VirtualMachine, VmError};

/// Status of a container running inside the VM
#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContainerStatus {
    /// Container ID (typically the short form from Podman)
    pub id: String,
    /// Container name (e.g., "opnble-project-id")
    pub name: String,
    /// Container state: "running", "exited", "paused", "created", etc.
    pub status: String,
    /// Forwarded port on the host, if any
    pub port: Option<u16>,
}

/// Resource usage statistics from the VM
#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ResourceStats {
    /// CPU usage as a percentage (0.0 - 100.0)
    pub cpu_percent: f32,
    /// Memory currently in use (in megabytes)
    pub memory_used_mb: u64,
    /// Total memory available (in megabytes)
    pub memory_total_mb: u64,
    /// Disk space available (in megabytes)
    pub disk_available_mb: u64,
    /// Total disk capacity (in megabytes)
    pub disk_total_mb: u64,
}

#[derive(Clone)]
pub struct VmGateway {
    vm: Arc<Mutex<Box<dyn VirtualMachine>>>,
    agent: Arc<AgentClient>,
}

impl VmGateway {
    /// Create a new `VmGateway` with platform-appropriate VM backend.
    ///
    /// Uses `VfkitVM` on macOS for fast boot via Virtualization.framework and
    /// consistent support for vsock + `VirtioFS`.
    ///
    /// The `AgentClient` connects to the guest agent via the vsock Unix socket
    /// created by the VM backend.
    pub fn new(
        app: &AppHandle,
        vm_image_path: &Path,
        repos_path: PathBuf,
    ) -> Result<Self, VmError> {
        let ssh_port = 2222; // Trait compatibility; vfkit path uses vsock internally.

        let vm: Box<dyn VirtualMachine> = Box::new(VfkitVM::new(
            app,
            vm_image_path.to_path_buf(),
            ssh_port,
            repos_path,
        )?);

        // Determine vsock socket path from the VM backend
        let socket_path = vm.vsock_socket().map_or_else(
            || {
                vm_image_path
                    .parent()
                    .unwrap_or(vm_image_path)
                    .join("agent.sock")
            },
            std::path::Path::to_path_buf,
        );

        let agent = Arc::new(AgentClient::new(socket_path));

        Ok(Self {
            vm: Arc::new(Mutex::new(vm)),
            agent,
        })
    }

    /// Start the VM and wait for the agent to become ready.
    ///
    /// The lock is released between `vm.start()` and the blocking wait so
    /// the tokio runtime isn't starved while we poll the vsock socket
    /// (up to 60 s of `thread::sleep`).
    pub async fn start(&self) -> Result<(), VmError> {
        // Start the VM and extract the vsock socket path while holding the lock.
        let socket_path = {
            let mut vm = self.vm.lock().await;
            vm.start()?;
            vm.vsock_socket()
                .map(std::path::Path::to_path_buf)
                .ok_or_else(|| VmError::ConnectionFailed {
                    reason: "no vsock socket exposed".to_string(),
                })?
        };
        // Lock released - do the blocking wait on a blocking thread.
        let path = socket_path.clone();
        tracing::debug!(socket = %path.display(), "waiting for agent vsock");
        tokio::task::spawn_blocking(move || {
            for i in 0..60 {
                let exists = path.exists();
                let connected = exists && std::os::unix::net::UnixStream::connect(&path).is_ok();
                tracing::trace!(
                    attempt = i,
                    max_attempts = 60,
                    exists,
                    connected,
                    "vsock readiness probe"
                );
                if connected {
                    tracing::info!("Agent reachable via vsock after {i}s");
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            Err(VmError::ConnectionFailed {
                reason: "agent not reachable after 60s".to_string(),
            })
        })
        .await
        .map_err(|e| VmError::LaunchFailed {
            reason: format!("cannot join readiness task: {e}"),
        })??;

        // Verify agent connectivity with retries. The vsock socket appears when
        // the guest binds, but the ttrpc server may not be fully started yet.
        let mut ping_ok = false;
        for attempt in 0..10 {
            match self.agent.ping().await {
                Ok(_) => {
                    tracing::info!("Agent ping succeeded on attempt {attempt}");
                    ping_ok = true;
                    break;
                }
                Err(e) => {
                    tracing::debug!("Agent ping attempt {attempt} failed: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                }
            }
        }
        if !ping_ok {
            let mut vm = self.vm.lock().await;
            let _ = vm.stop();
            return Err(VmError::ConnectionFailed {
                reason: "agent ping failed after 10 attempts".to_string(),
            });
        }

        Ok(())
    }

    /// Stop the VM and disconnect the agent.
    pub async fn stop(&self) -> Result<(), VmError> {
        self.agent.disconnect().await;
        let mut vm = self.vm.lock().await;
        vm.stop()
    }

    /// Get a reference to the `AgentClient` for direct RPC calls.
    pub fn agent(&self) -> &Arc<AgentClient> {
        &self.agent
    }

    /// Get the vsock socket path for port forwarding.
    pub fn vsock_socket_path(&self) -> &Path {
        self.agent.socket_path()
    }

    /// Get the status of a specific container by project ID.
    pub async fn container_status(&self, project_id: &str) -> Result<ContainerStatus, VmError> {
        let info = self.agent.container_status(project_id).await.map_err(|e| {
            VmError::ConnectionFailed {
                reason: format!("cannot query container status: {e}"),
            }
        })?;

        Ok(ContainerStatus {
            id: info.id,
            name: info.name,
            status: info.status,
            port: None,
        })
    }

    /// List all Opnble containers (running or stopped).
    pub async fn list_containers(&self) -> Result<Vec<ContainerStatus>, VmError> {
        let containers =
            self.agent
                .list_containers()
                .await
                .map_err(|e| VmError::ConnectionFailed {
                    reason: format!("cannot list containers: {e}"),
                })?;

        Ok(containers
            .into_iter()
            .map(|info| ContainerStatus {
                id: info.id,
                name: info.name,
                status: info.status,
                port: None,
            })
            .collect())
    }

    /// Run a shell command in a one-shot Alpine container and return (`exit_code`, logs).
    ///
    /// Used for ad-hoc debugging: starts a temporary container with the given
    /// command, polls until exit (max 60s), captures output, then cleans up.
    pub async fn exec(&self, cmd: &str) -> Result<(i32, String), VmError> {
        let project_id = "debug";

        let _result = self
            .agent
            .start_container(&super::agent_client::StartContainerParams {
                project_id,
                repo_path: "/tmp",
                port: 0,
                image: "node:20-alpine",
                cmd,
                internal_port: 0,
            })
            .await
            .map_err(|e| VmError::ConnectionFailed {
                reason: format!("cannot start debug container: {e}"),
            })?;

        // Poll until exit (max 60 seconds, 2s interval)
        let poll_interval = std::time::Duration::from_secs(2);
        let max_wait = std::time::Duration::from_mins(1);
        let start_time = std::time::Instant::now();
        loop {
            tokio::time::sleep(poll_interval).await;

            if start_time.elapsed() > max_wait {
                let _ = self.agent.stop_container(project_id).await;
                let _ = self.agent.remove_container(project_id).await;
                return Err(VmError::ConnectionFailed {
                    reason: "debug container timed out after 60 seconds".to_string(),
                });
            }

            let status = self.agent.container_status(project_id).await;
            let still_running = status.as_ref().is_ok_and(|info| info.status == "running");
            if !still_running {
                break;
            }
        }

        // Capture logs
        let logs = self
            .agent
            .stream_logs(project_id, 200)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(line, _stream)| line)
            .collect::<Vec<_>>()
            .join("\n");

        // Get exit code
        let exit_code = self
            .agent
            .project_health(project_id, 0)
            .await
            .map_or(-1, |h| h.exit_code);

        // Clean up
        let _ = self.agent.remove_container(project_id).await;

        Ok((exit_code, logs))
    }

    /// Get VM resource usage statistics (CPU, memory, and disk).
    pub async fn resource_stats(&self) -> Result<ResourceStats, VmError> {
        let vm_stats = self
            .agent
            .stats()
            .await
            .map_err(|e| VmError::ConnectionFailed {
                reason: format!("cannot query resource stats: {e}"),
            })?;

        Ok(ResourceStats {
            cpu_percent: vm_stats.cpu_percent,
            memory_used_mb: vm_stats.memory_used_mb,
            memory_total_mb: vm_stats.memory_total_mb,
            disk_available_mb: vm_stats.disk_available_mb,
            disk_total_mb: vm_stats.disk_total_mb,
        })
    }
}
