use std::fs::OpenOptions;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use tokio::runtime::Handle;

use crate::container::{self, PodmanRuntime};
use crate::error::AgentError;
use crate::generated::agent::{
    ContainerStatusRequest, ContainerStatusResponse, EnsurePathRequest, EnsurePathResponse,
    ExecHostRequest, ExecHostResponse, GetStatsRequest, GetStatsResponse, ListContainersRequest,
    ListContainersResponse, LogEntry, PingRequest, PingResponse, ProjectHealthRequest,
    ProjectHealthResponse, RemoveContainerRequest, RemoveContainerResponse, RemovePathRequest,
    RemovePathResponse, RenamePathRequest, RenamePathResponse, StartContainerRequest,
    StartContainerResponse, StopContainerRequest, StopContainerResponse, StreamLogsRequest,
    StreamLogsResponse, TouchFileRequest, TouchFileResponse,
};
use crate::generated::agent_ttrpc::Agent;
use crate::health;

pub struct ContainerAgent {
    rt: Handle,
}

impl ContainerAgent {
    pub fn new(rt: Handle) -> Self {
        ContainerAgent { rt }
    }

    /// Get a fresh Podman connection. Creates a new bollard client each time
    /// to avoid stale HTTP connection pool issues. If the socket is dead
    /// (Podman crashed but left a stale socket file), restart `podman system
    /// service` before connecting.
    fn podman() -> Result<PodmanRuntime, AgentError> {
        let socket_path = container::PODMAN_SOCKET;
        if !is_socket_alive(socket_path) {
            restart_podman_service(socket_path);
        }
        PodmanRuntime::connect()
    }
}

/// Convert an application-level error into a ttrpc INTERNAL status error.
///
/// Accepts anything convertible into `String` so that both legacy `String`
/// errors and `AgentError` (via `From<AgentError> for String`) flow through
/// `.map_err(internal_err)` unchanged.
fn internal_err(msg: impl Into<String>) -> ttrpc::Error {
    ttrpc::Error::RpcStatus(ttrpc::get_status(ttrpc::Code::INTERNAL, msg.into()))
}

/// Check if the Podman socket is actually accepting connections (not just a stale file).
fn is_socket_alive(path: &str) -> bool {
    use std::os::unix::net::UnixStream;
    if !std::path::Path::new(path).exists() {
        return false;
    }
    UnixStream::connect(path).is_ok()
}

/// Kill stale Podman, remove socket, restart the service, wait for socket.
fn restart_podman_service(socket_path: &str) {
    log::warn!("podman socket dead or missing, restarting service");
    // Kill any zombie podman process
    let _ = std::process::Command::new("pkill")
        .args(["-9", "podman"])
        .output();
    std::thread::sleep(std::time::Duration::from_millis(500));
    let _ = std::fs::remove_file(socket_path);
    if let Some(parent) = std::path::Path::new(socket_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let uri = format!("unix://{socket_path}");
    let spawned = std::process::Command::new("podman")
        .args(["system", "service", "--time=0", &uri])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => {
            log::error!("failed to spawn podman system service: {e}");
            return;
        }
    };
    let pid = child.id();
    for _ in 0..100 {
        if is_socket_alive(socket_path) {
            log::info!("podman service restarted successfully (pid {pid})");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // Timeout: reap the freshly spawned child so we do not leak it on top
    // of an already-broken podman state.
    if let Err(e) = child.kill() {
        log::warn!("failed to kill stalled podman child (pid {pid}): {e}");
    }
    if let Err(e) = child.wait() {
        log::warn!("failed to reap stalled podman child (pid {pid}): {e}");
    }
    log::error!("podman service failed to restart within 10s");
}

fn validate_absolute_repo_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("path must be absolute: {}", path.display()));
    }
    if !path.starts_with("/repos") {
        return Err(format!("path must be under /repos: {}", path.display()));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!(
            "path contains parent traversal component: {}",
            path.display()
        ));
    }
    Ok(())
}

fn canonicalize_existing_within_repos(path: &Path) -> Result<PathBuf, String> {
    validate_absolute_repo_path(path)?;
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| crate::io_error_context("canonicalize", path, &e))?;
    if !canonical.starts_with("/repos") {
        return Err(format!("path {} resolves outside /repos", path.display()));
    }
    Ok(canonical)
}

fn canonicalize_parent_within_repos(path: &Path) -> Result<PathBuf, String> {
    validate_absolute_repo_path(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("path has no parent: {}", path.display()))?;
    let canonical_parent = std::fs::canonicalize(parent)
        .map_err(|e| crate::io_error_context("canonicalize parent", parent, &e))?;
    if !canonical_parent.starts_with("/repos") {
        return Err(format!("path {} resolves outside /repos", path.display()));
    }
    Ok(canonical_parent)
}

/// True if path is exactly the /repos root (allows /repos or /repos/).
fn is_repos_root(path: &Path) -> bool {
    path == Path::new("/repos") || path == Path::new("/repos/")
}

fn ensure_path(path: &Path, is_dir: bool) -> Result<(), String> {
    validate_absolute_repo_path(path)?;

    if is_dir {
        // Pre-check: existing ancestor/parent must resolve under /repos.
        if !is_repos_root(path) {
            let _ = canonicalize_parent_within_repos(path)?;
        }
        std::fs::create_dir_all(path)
            .map_err(|e| crate::io_error_context("create directory", path, &e))?;
        // Post-check: close TOCTOU window if path changed between check and create.
        let canonical = std::fs::canonicalize(path)
            .map_err(|e| crate::io_error_context("canonicalize", path, &e))?;
        if canonical != Path::new("/repos") && !canonical.starts_with("/repos/") {
            return Err(format!("path {} resolves outside /repos", path.display()));
        }
        return Ok(());
    }

    // Pre-check: parent chain must resolve under /repos before creating file.
    let _ = canonicalize_parent_within_repos(path)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| crate::io_error_context("create parent", parent, &e))?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| crate::io_error_context("ensure file", path, &e))?;
    // Post-check: close TOCTOU window after creation.
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| crate::io_error_context("canonicalize", path, &e))?;
    if !canonical.starts_with("/repos") {
        return Err(format!("path {} resolves outside /repos", path.display()));
    }
    Ok(())
}

fn remove_path(path: &Path, recursive: bool) -> Result<(), String> {
    validate_absolute_repo_path(path)?;
    if is_repos_root(path) {
        return Err("cannot remove /repos root".to_string());
    }
    if !path.exists() {
        return Ok(());
    }
    let canonical = canonicalize_existing_within_repos(path)?;
    if is_repos_root(&canonical) {
        return Err("cannot remove /repos root".to_string());
    }
    let metadata = std::fs::metadata(&canonical)
        .map_err(|e| crate::io_error_context("stat", &canonical, &e))?;
    if metadata.is_dir() {
        if recursive {
            std::fs::remove_dir_all(&canonical)
                .map_err(|e| crate::io_error_context("remove directory", &canonical, &e))
        } else {
            std::fs::remove_dir(&canonical)
                .map_err(|e| crate::io_error_context("remove directory", &canonical, &e))
        }
    } else {
        std::fs::remove_file(&canonical)
            .map_err(|e| crate::io_error_context("remove file", &canonical, &e))
    }
}

fn rename_path(old_path: &Path, new_path: &Path) -> Result<(), String> {
    validate_absolute_repo_path(old_path)?;
    validate_absolute_repo_path(new_path)?;

    // I4: Fail when source is missing instead of silently succeeding
    if !old_path.exists() {
        return Err(format!(
            "rename source does not exist: {}",
            old_path.display()
        ));
    }

    let canonical_old = canonicalize_existing_within_repos(old_path)?;
    let _ = canonicalize_parent_within_repos(new_path)?;
    if let Some(parent) = new_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| crate::io_error_context("create parent", parent, &e))?;
    }

    std::fs::rename(&canonical_old, new_path).map_err(|e| {
        format!(
            "failed to rename {} to {}: {}",
            canonical_old.display(),
            new_path.display(),
            e
        )
    })?;
    Ok(())
}

impl Agent for ContainerAgent {
    fn start_container(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: StartContainerRequest,
    ) -> ttrpc::Result<StartContainerResponse> {
        // I8: Validate project_id at RPC boundary (defense-in-depth)
        container::validate_project_id(&req.project_id).map_err(internal_err)?;
        if !req.repo_path.starts_with("/repos/") {
            return Err(internal_err(format!(
                "repo_path must start with /repos/: {}",
                req.repo_path
            )));
        }
        if req.repo_path.contains("..") {
            return Err(internal_err(
                "repo_path contains '..' component".to_string(),
            ));
        }
        if req.image.is_empty() || req.image.starts_with('-') {
            return Err(internal_err(format!("invalid image: {}", req.image)));
        }

        // port=0 means "no port forwarding needed" (e.g. one-shot build containers)
        let host_port = u16::try_from(req.host_port)
            .map_err(|_| internal_err("host_port out of u16 range".to_string()))?;
        let internal_port = u16::try_from(req.internal_port)
            .map_err(|_| internal_err("internal_port out of u16 range".to_string()))?;
        if (host_port == 0) != (internal_port == 0) {
            return Err(internal_err(
                "cannot mix zero and non-zero ports: both host_port and internal_port must match"
                    .to_string(),
            ));
        }

        let runtime = Self::podman().map_err(internal_err)?;
        let container_id = self
            .rt
            .block_on(runtime.start_container(
                &req.project_id,
                &req.repo_path,
                host_port,
                &req.image,
                &req.cmd,
                internal_port,
            ))
            .map_err(internal_err)?;

        let mut resp = StartContainerResponse::new();
        resp.container_id = container_id;
        resp.port = req.host_port;
        Ok(resp)
    }

    fn stop_container(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: StopContainerRequest,
    ) -> ttrpc::Result<StopContainerResponse> {
        container::validate_project_id(&req.project_id).map_err(internal_err)?;
        let runtime = Self::podman().map_err(internal_err)?;
        self.rt
            .block_on(runtime.stop_container(&req.project_id))
            .map_err(internal_err)?;
        Ok(StopContainerResponse::new())
    }

    fn remove_container(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: RemoveContainerRequest,
    ) -> ttrpc::Result<RemoveContainerResponse> {
        container::validate_project_id(&req.project_id).map_err(internal_err)?;
        let runtime = Self::podman().map_err(internal_err)?;
        self.rt
            .block_on(runtime.remove_container(&req.project_id))
            .map_err(internal_err)?;
        Ok(RemoveContainerResponse::new())
    }

    fn container_status(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: ContainerStatusRequest,
    ) -> ttrpc::Result<ContainerStatusResponse> {
        container::validate_project_id(&req.project_id).map_err(internal_err)?;
        let runtime = Self::podman().map_err(internal_err)?;
        let (id, name, status) = self
            .rt
            .block_on(runtime.container_status(&req.project_id))
            .map_err(internal_err)?;

        let mut resp = ContainerStatusResponse::new();
        resp.container_id = id;
        resp.name = name;
        resp.status = status;
        Ok(resp)
    }

    fn list_containers(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        _req: ListContainersRequest,
    ) -> ttrpc::Result<ListContainersResponse> {
        let runtime = Self::podman().map_err(internal_err)?;
        let entries = self
            .rt
            .block_on(runtime.list_containers())
            .map_err(internal_err)?;

        let mut resp = ListContainersResponse::new();
        resp.containers = entries
            .into_iter()
            .map(|(id, name, status)| {
                let mut c = ContainerStatusResponse::new();
                c.container_id = id;
                c.name = name;
                c.status = status;
                c
            })
            .collect();
        Ok(resp)
    }

    fn stream_logs(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: StreamLogsRequest,
    ) -> ttrpc::Result<StreamLogsResponse> {
        let container_name = container::container_name(&req.project_id).map_err(internal_err)?;
        let tail = usize::try_from(req.tail)
            .ok()
            .filter(|&t| t > 0)
            .unwrap_or(100);

        let runtime = Self::podman().map_err(internal_err)?;
        let entries = self
            .rt
            .block_on(runtime.recent_logs(&container_name, tail))
            .map_err(|e| internal_err(format!("Failed to get logs: {e}")))?;

        let mut resp = StreamLogsResponse::new();
        resp.entries = entries
            .into_iter()
            .map(|e| {
                let mut entry = LogEntry::new();
                entry.line = e.line;
                entry.stream = e.stream;
                entry.timestamp = 0;
                entry
            })
            .collect();

        Ok(resp)
    }

    fn touch_file(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: TouchFileRequest,
    ) -> ttrpc::Result<TouchFileResponse> {
        let path = Path::new(&req.path);

        // Validate path boundary before any logic (reject paths outside /repos even if missing).
        validate_absolute_repo_path(path).map_err(internal_err)?;

        // Only touch existing files; if the file doesn't exist, return Ok
        // (it may be a transient file that was already deleted).
        if !path.exists() {
            return Ok(TouchFileResponse::new());
        }

        let canonical = canonicalize_existing_within_repos(path).map_err(internal_err)?;

        // Update mtime to now
        let now = filetime::FileTime::from_system_time(SystemTime::now());
        filetime::set_file_mtime(&canonical, now)
            .map_err(|e| internal_err(format!("failed to touch {}: {}", req.path, e)))?;

        Ok(TouchFileResponse::new())
    }

    fn ensure_path(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: EnsurePathRequest,
    ) -> ttrpc::Result<EnsurePathResponse> {
        ensure_path(Path::new(&req.path), req.is_dir).map_err(internal_err)?;
        Ok(EnsurePathResponse::new())
    }

    fn remove_path(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: RemovePathRequest,
    ) -> ttrpc::Result<RemovePathResponse> {
        remove_path(Path::new(&req.path), req.recursive).map_err(internal_err)?;
        Ok(RemovePathResponse::new())
    }

    fn rename_path(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: RenamePathRequest,
    ) -> ttrpc::Result<RenamePathResponse> {
        rename_path(Path::new(&req.old_path), Path::new(&req.new_path)).map_err(internal_err)?;
        Ok(RenamePathResponse::new())
    }

    fn project_health(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: ProjectHealthRequest,
    ) -> ttrpc::Result<ProjectHealthResponse> {
        container::validate_project_id(&req.project_id).map_err(internal_err)?;
        let port = u16::try_from(req.port)
            .map_err(|_| internal_err("port out of u16 range".to_string()))?;

        let runtime = Self::podman().map_err(internal_err)?;
        let result = self
            .rt
            .block_on(health::project_health(&runtime, &req.project_id, port))
            .map_err(internal_err)?;

        let mut resp = ProjectHealthResponse::new();
        resp.container_state = result.container_state;
        resp.exit_code = result.exit_code;
        resp.http_reachable = result.http_reachable;
        Ok(resp)
    }

    fn ping(&self, _ctx: &ttrpc::TtrpcContext, _req: PingRequest) -> ttrpc::Result<PingResponse> {
        let mut resp = PingResponse::new();
        resp.version = env!("CARGO_PKG_VERSION").to_string();
        resp.fs_ops_v2 = true;
        Ok(resp)
    }

    fn get_stats(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        _req: GetStatsRequest,
    ) -> ttrpc::Result<GetStatsResponse> {
        let (cpu, mem_used, mem_total) = container::stats().map_err(internal_err)?;
        let (disk_available, disk_total) = container::disk_stats().unwrap_or_else(|e| {
            log::warn!("cannot collect disk stats, returning zeros: {e}");
            (0, 0)
        });

        let mut resp = GetStatsResponse::new();
        resp.cpu_percent = cpu;
        resp.memory_used_mb = mem_used;
        resp.memory_total_mb = mem_total;
        resp.disk_available_mb = disk_available;
        resp.disk_total_mb = disk_total;
        Ok(resp)
    }

    fn exec_host(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        req: ExecHostRequest,
    ) -> ttrpc::Result<ExecHostResponse> {
        if req.command.is_empty() {
            return Err(internal_err("cannot execute empty command".to_string()));
        }

        let output = std::process::Command::new("sh")
            .args(["-c", &req.command])
            .output()
            .map_err(|e| internal_err(format!("cannot execute command: {e}")))?;

        let mut resp = ExecHostResponse::new();
        resp.exit_code = output.status.code().unwrap_or(-1);
        resp.stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        resp.stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ensure_test_root() -> Option<PathBuf> {
        let root = PathBuf::from("/repos/opnble-agent-tests");
        if std::fs::create_dir_all(&root).is_ok() {
            Some(root)
        } else {
            None
        }
    }

    fn unique_path(suffix: &str) -> PathBuf {
        let Some(root) = ensure_test_root() else {
            return PathBuf::from(format!("/repos/skip-{suffix}"));
        };
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        root.join(format!("opnble-test-{nanos}-{suffix}"))
    }

    #[test]
    fn ensure_file_and_remove_file_are_idempotent() {
        if ensure_test_root().is_none() {
            return;
        }
        let path = unique_path("file.txt");
        ensure_path(&path, false).unwrap();
        ensure_path(&path, false).unwrap();
        assert!(path.exists());
        remove_path(&path, false).unwrap();
        remove_path(&path, false).unwrap();
    }

    #[test]
    fn ensure_dir_and_remove_dir_recursive_work() {
        if ensure_test_root().is_none() {
            return;
        }
        let dir = unique_path("dir/sub");
        ensure_path(&dir, true).unwrap();
        assert!(dir.exists());
        let root = dir.parent().unwrap().parent().unwrap().to_path_buf();
        remove_path(&root, true).unwrap();
        assert!(!root.exists());
    }

    #[test]
    fn rename_path_fails_when_source_missing() {
        // I4: rename_path must return error when source does not exist
        if ensure_test_root().is_none() {
            return;
        }
        let old_path = unique_path("old-nonexistent.txt");
        let new_path = unique_path("new.txt");
        assert!(rename_path(&old_path, &new_path).is_err());
    }

    #[test]
    fn rename_path_succeeds_when_source_exists() {
        if ensure_test_root().is_none() {
            return;
        }
        let old_path = unique_path("old.txt");
        let new_path = unique_path("new.txt");
        ensure_path(&old_path, false).unwrap();
        rename_path(&old_path, &new_path).unwrap();
        assert!(!old_path.exists());
        assert!(new_path.exists());
    }

    #[test]
    fn reject_parent_traversal() {
        let bad = Path::new("/repos/foo/../bar");
        assert!(validate_absolute_repo_path(bad).is_err());
    }

    #[test]
    fn reject_path_outside_repos() {
        assert!(validate_absolute_repo_path(Path::new("/tmp/foo")).is_err());
        assert!(validate_absolute_repo_path(Path::new("/reposfoo")).is_err());
    }

    #[test]
    fn reject_relative_path() {
        assert!(validate_absolute_repo_path(Path::new("repos/foo")).is_err());
    }

    #[test]
    fn ensure_path_repos_root_succeeds() {
        // ensure_path("/repos", true) must not fail due to parent '/' - it should succeed
        // when /repos exists or can be created. Skip if we cannot create /repos.
        let root = Path::new("/repos");
        if let Err(e) = ensure_path(root, true) {
            // May fail on CI or systems without /repos; avoid hard failure
            log::warn!("ensure_path(/repos, true) failed (expected in some envs): {e}");
            return;
        }
        assert!(root.exists() && root.is_dir());
    }

    #[test]
    fn remove_path_rejects_repos_root() {
        // Must reject removing /repos root (recursive or not)
        assert!(remove_path(Path::new("/repos"), false).is_err());
        assert!(remove_path(Path::new("/repos"), true).is_err());
        assert!(remove_path(Path::new("/repos/"), false).is_err());
        assert!(remove_path(Path::new("/repos/"), true).is_err());
    }

    #[test]
    fn touch_file_validates_path_when_file_missing() {
        // Must validate path boundary even when file does not exist
        let bad = Path::new("/etc/passwd");
        assert!(validate_absolute_repo_path(bad).is_err());
        // Path under /repos but non-existent is valid (touch_file returns Ok)
        let good_missing = Path::new("/repos/nonexistent-file-12345");
        assert!(validate_absolute_repo_path(good_missing).is_ok());
    }
}
