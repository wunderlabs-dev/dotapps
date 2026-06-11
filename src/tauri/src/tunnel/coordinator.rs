//! Tunnel lifecycle coordination
//!
//! Coordinates API calls and cloudflared child process spawning/killing
//! for per-project tunnel sharing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::constants::tunnel;
use crate::error::AppError;
use crate::projects::store::ProjectStore;
use crate::projects::types::ProjectId;

use super::api::{CreateTunnelResponse, TunnelApiClient};

const TUNNEL_READY_TIMEOUT: Duration = Duration::from_secs(45);

/// Active tunnel state for a single project
struct ActiveTunnel {
    tunnel_id: String,
    tunnel_url: String,
    process: Child,
    live: bool,
    /// Stderr-watching task; aborted by `teardown_tunnel`.
    ready_watcher: JoinHandle<()>,
}

/// Coordinates cloudflared tunnel processes and API interactions for
/// sharing running projects via public URLs.
pub struct TunnelCoordinator {
    api: TunnelApiClient,
    cloudflared_path: Mutex<PathBuf>,
    active: Arc<Mutex<HashMap<String, ActiveTunnel>>>,
}

impl TunnelCoordinator {
    pub fn new(cloudflared_path: PathBuf) -> Result<Self, AppError> {
        Ok(Self {
            api: TunnelApiClient::new(tunnel::API_BASE_URL)?,
            cloudflared_path: Mutex::new(cloudflared_path),
            active: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Override the cloudflared binary path.
    ///
    /// Called from `setup()` once `resource_dir()` is available, replacing the
    /// initial dev-mode placeholder with the bundled binary location.
    pub async fn set_cloudflared_path(&self, path: PathBuf) {
        *self.cloudflared_path.lock().await = path;
    }

    /// Clean up orphaned tunnels from a previous session.
    ///
    /// On app startup, projects may have `tunnel_id` set from a previous run
    /// where cloudflared died (crash, kill -9). This deletes those tunnels
    /// via the API and clears the project records.
    pub async fn cleanup_orphaned_tunnels(&self, github_token: &str, store: &dyn ProjectStore) {
        let projects = match store.list() {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("cannot list projects for tunnel cleanup: {e}");
                return;
            }
        };

        for project in projects {
            let Some(ref tunnel_id) = project.tunnel_id else {
                continue;
            };

            tracing::info!(
                "cleaning up orphaned tunnel {tunnel_id} for project {}",
                project.id
            );

            if let Err(e) = self.api.delete_tunnel(tunnel_id, github_token).await {
                tracing::warn!("cannot delete orphaned tunnel {tunnel_id}: {e}");
            }

            Self::clear_tunnel_record(store, &project.id);
        }
    }

    /// Share a project: create tunnel via API, spawn cloudflared, return public URL.
    ///
    /// Fails if the project is already shared (call `unshare` first).
    pub async fn share(
        &self,
        project_id: &ProjectId,
        project_name: &str,
        host_port: u16,
        github_token: &str,
        store: &dyn ProjectStore,
    ) -> Result<String, AppError> {
        let id_str = project_id.as_str();

        // If already shared, return the existing URL (idempotent)
        {
            let active = self.active.lock().await;
            if let Some(existing) = active.get(id_str) {
                return Ok(existing.tunnel_url.clone());
            }
        }

        // Resolve and validate cloudflared binary path
        let cloudflared_path = self.cloudflared_path.lock().await.clone();
        let resolved = resolve_cloudflared(&cloudflared_path)?;

        // Create tunnel via Worker API
        let CreateTunnelResponse {
            tunnel_id,
            tunnel_token,
            url,
        } = self
            .api
            .create_tunnel(project_name, id_str, host_port, github_token)
            .await?;

        // Remotely managed tunnels load ingress from the Cloudflare API.
        // The --url and --http-host-header flags are ignored with --token.
        let mut child = Command::new(&resolved)
            .args(["tunnel", "run", "--token", &tunnel_token])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::TunnelFailed {
                reason: format!("cannot start cloudflared: {e}"),
            })?;

        tracing::info!(
            "cloudflared started for project {id_str} (pid {})",
            child.id().unwrap_or(0)
        );

        // Watch cloudflared stderr for "Registered tunnel connection" in
        // background. When detected, mark the tunnel as live.
        let stderr = child.stderr.take();
        let active_ref = Arc::clone(&self.active);
        let id_for_watcher = id_str.to_string();
        let ready_watcher = tokio::spawn(async move {
            watch_cloudflared_ready(stderr, active_ref, id_for_watcher).await;
        });

        // Persist tunnel info on the project record
        if let Ok(mut project) = store.get(project_id) {
            project.tunnel_id = Some(tunnel_id.clone());
            project.tunnel_url = Some(url.clone());
            if let Err(e) = store.update(project) {
                tracing::error!("cannot persist tunnel info for {id_str}: {e}");
            }
        }

        // Track active tunnel (starts as not live, watcher updates it)
        self.active.lock().await.insert(
            id_str.to_string(),
            ActiveTunnel {
                tunnel_id: tunnel_id.clone(),
                tunnel_url: url.clone(),
                process: child,
                live: false,
                ready_watcher,
            },
        );

        if !wait_for_tunnel_live(&self.active, id_str, TUNNEL_READY_TIMEOUT).await {
            self.abort_failed_share(id_str, project_id, github_token, store)
                .await;
            return Err(AppError::TunnelFailed {
                reason: "tunnel did not connect to Cloudflare in time".into(),
            });
        }

        if !crate::projects::orchestrator::probe_http_health(host_port).await {
            self.abort_failed_share(id_str, project_id, github_token, store)
                .await;
            return Err(AppError::TunnelFailed {
                reason: format!(
                    "dev server is not reachable on localhost:{host_port} for tunnel origin"
                ),
            });
        }

        Ok(url)
    }

    async fn abort_failed_share(
        &self,
        id_str: &str,
        project_id: &ProjectId,
        github_token: &str,
        store: &dyn ProjectStore,
    ) {
        let tunnel = {
            let mut active = self.active.lock().await;
            active.remove(id_str)
        };
        if let Some(tunnel) = tunnel {
            self.teardown_tunnel(id_str, tunnel, github_token).await;
        }
        Self::clear_tunnel_record(store, project_id);
    }

    /// Unshare a project: kill cloudflared, delete tunnel via API, clear project record.
    pub async fn unshare(
        &self,
        project_id: &ProjectId,
        github_token: &str,
        store: &dyn ProjectStore,
    ) -> Result<(), AppError> {
        let id_str = project_id.as_str();
        let tunnel = {
            let mut active = self.active.lock().await;
            active.remove(id_str)
        };

        let Some(tunnel) = tunnel else {
            // Not shared, nothing to do
            return Ok(());
        };

        self.teardown_tunnel(id_str, tunnel, github_token).await;

        Self::clear_tunnel_record(store, project_id);

        tracing::info!("tunnel stopped for project {id_str}");
        Ok(())
    }

    /// Get the tunnel URL for a project (if shared).
    pub async fn tunnel_url(&self, project_id: &ProjectId) -> Option<String> {
        let active = self.active.lock().await;
        active
            .get(project_id.as_str())
            .map(|t| t.tunnel_url.clone())
    }

    /// Check if the tunnel is live (cloudflared registered with Cloudflare).
    pub async fn is_tunnel_live(&self, project_id: &ProjectId) -> bool {
        let active = self.active.lock().await;
        active.get(project_id.as_str()).is_some_and(|t| t.live)
    }

    /// Stop all active tunnels and clear their project records (for graceful app shutdown).
    /// Stop all active tunnels. Uses empty token for API calls (process kill
    /// still works; API delete is best-effort during shutdown).
    pub async fn stop_all(&self, store: &dyn ProjectStore) {
        let tunnels: Vec<_> = {
            let mut active = self.active.lock().await;
            active.drain().collect()
        };
        for (id_str, tunnel) in tunnels {
            self.teardown_tunnel(&id_str, tunnel, "").await;

            if let Ok(project_id) = ProjectId::new(&id_str) {
                Self::clear_tunnel_record(store, &project_id);
            }
        }
    }

    /// Clear `tunnel_id` and `tunnel_url` from a project's persisted record.
    fn clear_tunnel_record(store: &dyn ProjectStore, project_id: &ProjectId) {
        if let Ok(mut project) = store.get(project_id) {
            project.tunnel_id = None;
            project.tunnel_url = None;
            if let Err(e) = store.update(project) {
                tracing::error!("cannot clear tunnel info for {}: {e}", project_id.as_str());
            }
        }
    }

    /// Kill the cloudflared process and delete the tunnel via API.
    async fn teardown_tunnel(&self, id_str: &str, mut tunnel: ActiveTunnel, github_token: &str) {
        tunnel.ready_watcher.abort();
        if let Err(e) = tunnel.process.kill().await {
            tracing::warn!("cannot kill cloudflared for {id_str}: {e}");
        }
        let _ = tunnel.process.wait().await;
        if let Err(e) = self
            .api
            .delete_tunnel(&tunnel.tunnel_id, github_token)
            .await
        {
            tracing::warn!("cannot delete tunnel for {id_str}: {e}");
        }
    }
}

/// Watch cloudflared's stderr for "Registered tunnel connection" and mark live.
///
/// Cloudflared logs to stderr when it establishes QUIC connections to
/// Cloudflare's edge. We watch for this event-driven signal instead of
/// polling the tunnel URL.
async fn watch_cloudflared_ready(
    stderr: Option<tokio::process::ChildStderr>,
    active: Arc<Mutex<HashMap<String, ActiveTunnel>>>,
    project_id: String,
) {
    let Some(stderr) = stderr else {
        tracing::warn!("cloudflared stderr not available for {project_id}");
        return;
    };

    let reader = BufReader::new(stderr);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        tracing::debug!("cloudflared [{project_id}]: {line}");

        if line.contains("Registered tunnel connection") {
            tracing::info!("tunnel live for project {project_id}");
            let mut map = active.lock().await;
            if let Some(tunnel) = map.get_mut(&project_id) {
                tunnel.live = true;
            }
            // Keep reading to log subsequent messages, but liveness is set
        }
    }

    tracing::debug!("cloudflared stderr closed for {project_id}");
}

async fn wait_for_tunnel_live(
    active: &Arc<Mutex<HashMap<String, ActiveTunnel>>>,
    project_id: &str,
    timeout: Duration,
) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        let live = {
            let map = active.lock().await;
            map.get(project_id).is_some_and(|tunnel| tunnel.live)
        };
        if live {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

/// Resolve the cloudflared binary path.
///
/// If the path is absolute and exists on disk, use it directly (production bundle).
/// If it is a bare name (e.g. "cloudflared"), search PATH so that dev-mode
/// installations (brew, manual) are found without an absolute path.
fn resolve_cloudflared(candidate: &Path) -> Result<PathBuf, AppError> {
    // Absolute or relative path that exists: use as-is
    if candidate.exists() {
        return Ok(candidate.to_path_buf());
    }

    // Bare binary name: search PATH
    if candidate.components().count() == 1 {
        if let Some(name) = candidate.to_str() {
            if let Some(found) = find_in_path(name) {
                return Ok(found);
            }
        }
    }

    Err(AppError::TunnelFailed {
        reason: format!(
            "cannot find cloudflared binary at {} (also not in PATH)",
            candidate.display()
        ),
    })
}

/// Search PATH for a binary by name (platform-aware).
fn find_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var("PATH").ok()?;
    let separator = if cfg!(windows) { ';' } else { ':' };

    for dir in path_var.split(separator) {
        let candidate = Path::new(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        // On Windows, also try with .exe extension
        if cfg!(windows) {
            let with_exe = Path::new(dir).join(format!("{name}.exe"));
            if with_exe.is_file() {
                return Some(with_exe);
            }
        }
    }

    None
}

/// Resolve the bundled cloudflared sidecar path.
///
/// Returns `None` when the binary is not where we expect it (dev runs that
/// skipped `prepare-resources.sh`, or a corrupted bundle). Callers fall back
/// to PATH lookup so the experience degrades gracefully.
pub fn bundled_cloudflared_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri_plugin_shell::ShellExt;
    let cmd: std::process::Command = app.shell().sidecar("cloudflared").ok()?.into();
    let path = PathBuf::from(cmd.get_program());
    path.exists().then_some(path)
}
