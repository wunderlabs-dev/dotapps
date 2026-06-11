//! Host-side ttrpc client for communicating with the guest agent.
//!
//! Connects to the guest agent via a Unix socket (bridged from vsock by vfkit).
//! This replaces the SSH-based command execution with type-safe ttrpc RPCs.
//!
//! The generated ttrpc client is synchronous, so we bridge into async context
//! using `tokio::task::spawn_blocking` for each RPC call.

use std::path::{Path, PathBuf};
use std::time::Duration;

use opnble_agent::generated::agent;
use opnble_agent::generated::agent_ttrpc::AgentClient as TtrpcAgentClient;
use tokio::sync::Mutex;

use crate::error::AppError;

/// Container lifecycle request parameters for `start_container`.
///
/// Groups the 6 parameters needed to launch a container into a single struct,
/// following the "4+ params is a struct" convention.
pub struct StartContainerParams<'params> {
    pub project_id: &'params str,
    pub repo_path: &'params str,
    pub port: u16,
    pub image: &'params str,
    pub cmd: &'params str,
    pub internal_port: u16,
}

/// Container identity returned by `container_status`.
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub status: String,
}

/// Project health status returned by `project_health`.
pub struct ProjectHealthInfo {
    pub container_state: String,
    pub exit_code: i32,
    pub http_reachable: bool,
}

/// VM resource usage returned by `stats`.
pub struct VmResourceStats {
    pub cpu_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub disk_available_mb: u64,
    pub disk_total_mb: u64,
}

/// Short timeout for lightweight RPCs (ping, `touch_file`).
const RPC_TIMEOUT_SHORT: Duration = Duration::from_secs(5);
/// Medium timeout for status/list/stats RPCs.
const RPC_TIMEOUT_MEDIUM: Duration = Duration::from_secs(30);
/// Long timeout for container lifecycle RPCs (start, stop, remove)
/// that may trigger image pulls or cleanup.
const RPC_TIMEOUT_LONG: Duration = Duration::from_mins(2);

/// Client for communicating with the guest agent via ttrpc over Unix socket.
///
/// The Unix socket is created by vfkit as a bridge to the VM's vsock device.
/// All container operations (start, stop, logs, etc.) go through this client.
pub struct AgentClient {
    socket_path: PathBuf,
    inner: Mutex<Option<TtrpcAgentClient>>,
    fs_ops_v2: Mutex<Option<bool>>,
}

/// Build a ttrpc context with the given timeout.
///
/// Centralizes the Duration -> nanoseconds conversion required by ttrpc.
/// Durations longer than `i64::MAX` nanoseconds (~292 years) are clamped;
/// this keeps the function total without a panic path.
/// If ttrpc ever adds `with_duration`, swap this one-liner.
fn rpc_context(timeout: Duration) -> ttrpc::context::Context {
    let nanos = i64::try_from(timeout.as_nanos()).unwrap_or(i64::MAX);
    ttrpc::context::with_timeout(nanos)
}

impl AgentClient {
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            inner: Mutex::new(None),
            fs_ops_v2: Mutex::new(None),
        }
    }

    /// Get the vsock socket path.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Get a connected client, reconnecting if needed.
    ///
    /// The lock is intentionally held across the `spawn_blocking` await
    /// during connect. The rust-backend rule says "never hold Tokio Mutex
    /// guards across `.await`"; we accept that local cost here because the
    /// alternative (drop the guard, connect, re-acquire) opens a TOCTOU
    /// where `clear_connection()` racing this task could discard the
    /// freshly-built client and leave us serving an old one. Connect runs
    /// at most once per session, so the contention window is small.
    async fn connected_client(&self) -> Result<TtrpcAgentClient, AppError> {
        let mut guard = self.inner.lock().await;
        if let Some(client) = guard.as_ref() {
            return Ok(client.clone());
        }
        let sockaddr = format!("unix://{}", self.socket_path.display());
        let client = tokio::task::spawn_blocking(move || ttrpc::Client::connect(&sockaddr))
            .await?
            .map_err(|e| AppError::VmStartFailed {
                reason: format!("cannot connect to agent: {e}"),
            })?;
        let agent = TtrpcAgentClient::new(client);
        let cloned = agent.clone();
        *guard = Some(agent);
        Ok(cloned)
    }

    /// Clear the cached connection so the next call reconnects.
    ///
    /// Called automatically on RPC failure so a dead connection
    /// (e.g., after an agent crash) doesn't poison all future calls.
    async fn clear_connection(&self) {
        let mut guard = self.inner.lock().await;
        *guard = None;
    }

    async fn set_fs_ops_v2(&self, supported: bool) {
        let mut guard = self.fs_ops_v2.lock().await;
        *guard = Some(supported);
    }

    async fn clear_fs_ops_v2(&self) {
        let mut guard = self.fs_ops_v2.lock().await;
        *guard = None;
    }

    /// Convert an RPC error into `AppError`, clearing cached state so the next
    /// call reconnects instead of reusing a potentially dead connection.
    async fn rpc_failed(&self, e: ttrpc::Error) -> AppError {
        self.clear_connection().await;
        self.clear_fs_ops_v2().await;
        AppError::from(e)
    }

    /// Send a ping to verify the agent is alive.
    pub async fn ping(&self) -> Result<String, AppError> {
        let client = self.connected_client().await?;
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_SHORT);
            let req = agent::PingRequest::new();
            client.ping(ctx, &req)
        })
        .await?;
        match result {
            Ok(resp) => {
                self.set_fs_ops_v2(resp.fs_ops_v2).await;
                Ok(resp.version)
            }
            Err(e) => Err(self.rpc_failed(e).await),
        }
    }

    pub async fn fs_ops_v2_supported(&self) -> Result<bool, AppError> {
        let cached = *self.fs_ops_v2.lock().await;
        if let Some(supported) = cached {
            return Ok(supported);
        }
        let _ = self.ping().await?;
        Ok(self.fs_ops_v2.lock().await.unwrap_or(false))
    }

    /// Start a container in the VM.
    pub async fn start_container(
        &self,
        params: &StartContainerParams<'_>,
    ) -> Result<(String, u16), AppError> {
        let client = self.connected_client().await?;
        let mut req = agent::StartContainerRequest::new();
        req.project_id = params.project_id.to_string();
        req.repo_path = params.repo_path.to_string();
        req.host_port = u32::from(params.port);
        req.image = params.image.to_string();
        req.cmd = params.cmd.to_string();
        req.internal_port = u32::from(params.internal_port);
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_LONG);
            client.start_container(ctx, &req)
        })
        .await?;
        match result {
            Ok(resp) => {
                let port = u16::try_from(resp.port).map_err(|_| AppError::ContainerFailed {
                    reason: format!("agent returned invalid port: {}", resp.port),
                })?;
                Ok((resp.container_id, port))
            }
            Err(e) => Err(self.rpc_failed(e).await),
        }
    }

    /// Stop a container in the VM.
    pub async fn stop_container(&self, project_id: &str) -> Result<(), AppError> {
        let client = self.connected_client().await?;
        let mut req = agent::StopContainerRequest::new();
        req.project_id = project_id.to_string();
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_LONG);
            client.stop_container(ctx, &req)
        })
        .await?;
        if let Err(e) = result {
            return Err(self.rpc_failed(e).await);
        }
        Ok(())
    }

    /// Remove a container in the VM.
    pub async fn remove_container(&self, project_id: &str) -> Result<(), AppError> {
        let client = self.connected_client().await?;
        let mut req = agent::RemoveContainerRequest::new();
        req.project_id = project_id.to_string();
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_LONG);
            client.remove_container(ctx, &req)
        })
        .await?;
        if let Err(e) = result {
            return Err(self.rpc_failed(e).await);
        }
        Ok(())
    }

    /// Get container status.
    pub async fn container_status(&self, project_id: &str) -> Result<ContainerInfo, AppError> {
        let client = self.connected_client().await?;
        let mut req = agent::ContainerStatusRequest::new();
        req.project_id = project_id.to_string();
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_MEDIUM);
            client.container_status(ctx, &req)
        })
        .await?;
        match result {
            Ok(resp) => Ok(ContainerInfo {
                id: resp.container_id,
                name: resp.name,
                status: resp.status,
            }),
            Err(e) => Err(self.rpc_failed(e).await),
        }
    }

    /// List all containers.
    pub async fn list_containers(&self) -> Result<Vec<ContainerInfo>, AppError> {
        let client = self.connected_client().await?;
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_MEDIUM);
            let req = agent::ListContainersRequest::new();
            client.list_containers(ctx, &req)
        })
        .await?;
        match result {
            Ok(resp) => {
                let containers = resp
                    .containers
                    .into_iter()
                    .map(|c| ContainerInfo {
                        id: c.container_id,
                        name: c.name,
                        status: c.status,
                    })
                    .collect();
                Ok(containers)
            }
            Err(e) => Err(self.rpc_failed(e).await),
        }
    }

    /// Get recent log lines from a container.
    ///
    /// Returns Vec of (line, stream) tuples where stream is "stdout" or "stderr".
    pub async fn stream_logs(
        &self,
        project_id: &str,
        tail: u32,
    ) -> Result<Vec<(String, String)>, AppError> {
        let client = self.connected_client().await?;
        let mut req = agent::StreamLogsRequest::new();
        req.project_id = project_id.to_string();
        req.tail = tail;
        req.follow = false;
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_MEDIUM);
            client.stream_logs(ctx, &req)
        })
        .await?;
        match result {
            Ok(resp) => Ok(resp
                .entries
                .into_iter()
                .map(|e| (e.line, e.stream))
                .collect()),
            Err(e) => Err(self.rpc_failed(e).await),
        }
    }

    /// Get VM resource stats (CPU, memory, and disk).
    pub async fn stats(&self) -> Result<VmResourceStats, AppError> {
        let client = self.connected_client().await?;
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_MEDIUM);
            let req = agent::GetStatsRequest::new();
            client.get_stats(ctx, &req)
        })
        .await?;
        match result {
            Ok(resp) => Ok(VmResourceStats {
                cpu_percent: resp.cpu_percent,
                memory_used_mb: resp.memory_used_mb,
                memory_total_mb: resp.memory_total_mb,
                disk_available_mb: resp.disk_available_mb,
                disk_total_mb: resp.disk_total_mb,
            }),
            Err(e) => Err(self.rpc_failed(e).await),
        }
    }

    /// Get project health status (container state, exit code, HTTP reachability).
    pub async fn project_health(
        &self,
        project_id: &str,
        port: u16,
    ) -> Result<ProjectHealthInfo, AppError> {
        let client = self.connected_client().await?;
        let mut req = agent::ProjectHealthRequest::new();
        req.project_id = project_id.to_string();
        req.port = u32::from(port);
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_MEDIUM);
            client.project_health(ctx, &req)
        })
        .await?;
        match result {
            Ok(resp) => Ok(ProjectHealthInfo {
                container_state: resp.container_state,
                exit_code: resp.exit_code,
                http_reachable: resp.http_reachable,
            }),
            Err(e) => Err(self.rpc_failed(e).await),
        }
    }

    /// Touch a file in the VM to trigger inotify for hot reload.
    pub async fn touch_file(&self, path: &str) -> Result<(), AppError> {
        let client = self.connected_client().await?;
        let mut req = agent::TouchFileRequest::new();
        req.path = path.to_string();
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_SHORT);
            client.touch_file(ctx, &req)
        })
        .await?;
        if let Err(e) = result {
            return Err(self.rpc_failed(e).await);
        }
        Ok(())
    }

    pub async fn ensure_path(&self, path: &str, is_dir: bool) -> Result<(), AppError> {
        let client = self.connected_client().await?;
        let mut req = agent::EnsurePathRequest::new();
        req.path = path.to_string();
        req.is_dir = is_dir;
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_SHORT);
            client.ensure_path(ctx, &req)
        })
        .await?;
        if let Err(e) = result {
            return Err(self.rpc_failed(e).await);
        }
        Ok(())
    }

    pub async fn remove_path(&self, path: &str, recursive: bool) -> Result<(), AppError> {
        let client = self.connected_client().await?;
        let mut req = agent::RemovePathRequest::new();
        req.path = path.to_string();
        req.recursive = recursive;
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_SHORT);
            client.remove_path(ctx, &req)
        })
        .await?;
        if let Err(e) = result {
            return Err(self.rpc_failed(e).await);
        }
        Ok(())
    }

    pub async fn rename_path(&self, old_path: &str, new_path: &str) -> Result<(), AppError> {
        let client = self.connected_client().await?;
        let mut req = agent::RenamePathRequest::new();
        req.old_path = old_path.to_string();
        req.new_path = new_path.to_string();
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_SHORT);
            client.rename_path(ctx, &req)
        })
        .await?;
        if let Err(e) = result {
            return Err(self.rpc_failed(e).await);
        }
        Ok(())
    }

    /// Run a shell command directly on the VM host (not inside a container).
    ///
    /// Returns (`exit_code`, `stdout`, `stderr`). Useful for debugging VM-level
    /// issues when Podman may be crashed or unavailable.
    pub async fn exec_host(&self, command: &str) -> Result<(i32, String, String), AppError> {
        let client = self.connected_client().await?;
        let mut req = agent::ExecHostRequest::new();
        req.command = command.to_string();
        let result = tokio::task::spawn_blocking(move || {
            let ctx = rpc_context(RPC_TIMEOUT_LONG);
            client.exec_host(ctx, &req)
        })
        .await?;
        match result {
            Ok(resp) => Ok((resp.exit_code, resp.stdout, resp.stderr)),
            Err(e) => Err(self.rpc_failed(e).await),
        }
    }

    /// Disconnect from the agent (call on VM shutdown).
    ///
    /// Releases the inner lock before acquiring `fs_ops_v2` to avoid
    /// lock ordering inconsistency with other code paths.
    pub async fn disconnect(&self) {
        {
            let mut guard = self.inner.lock().await;
            *guard = None;
        }
        self.clear_fs_ops_v2().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_client_constructs() {
        let _ = AgentClient::new(PathBuf::from("/tmp/test.sock"));
    }

    #[tokio::test]
    async fn disconnect_releases_locks_without_deadlock() {
        let client = AgentClient::new(PathBuf::from("/tmp/test.sock"));
        client.disconnect().await;
    }
}
