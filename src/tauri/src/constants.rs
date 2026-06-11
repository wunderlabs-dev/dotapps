//! Centralized constants for paths, containers, events, ports, and keyring
//!
//! This module provides a single source of truth for all magic strings and
//! configuration values used throughout the application.

/// Container-related constants
pub mod containers {
    /// Node.js image from Docker Hub
    pub const NODE_IMAGE: &str = "node:20-alpine";

    /// Bundled image name (pre-loaded during setup)
    #[cfg(target_os = "linux")]
    pub const BUNDLED_IMAGE: &str = "opnble-node:20-alpine";

    /// Prefix for all Opnble container names
    pub const NAME_PREFIX: &str = "opnble-";

    /// Internal port that Node.js dev servers listen on (Next.js default).
    /// Used by Linux (PodmanRuntime) and Windows (WSL) where containers use
    /// port bindings. macOS uses host networking with dynamic port instead.
    #[cfg(not(target_os = "macos"))]
    pub const INTERNAL_PORT: u16 = 3000;

    /// Smart npm install: only runs when `node_modules` is missing or
    /// `package.json`/`package-lock.json` is newer than the last install.
    /// Exits after install completes (one-shot container).
    #[inline]
    pub fn install_cmd() -> String {
        "if [ ! -d node_modules ] || [ package.json -nt node_modules/.package-lock.json ] || { [ -f package-lock.json ] && [ package-lock.json -nt node_modules/.package-lock.json ]; }; then npm install; fi".to_string()
    }

    /// Start the dev server on the given port, listening on all interfaces.
    /// Assumes dependencies are already installed (run `install_cmd` first).
    #[inline]
    pub fn dev_cmd(port: u16) -> String {
        format!("npm run dev -- --port {port} --host")
    }

    /// Generate a container name for a project
    #[inline]
    pub fn name(project_id: &str) -> String {
        format!("{NAME_PREFIX}{project_id}")
    }

    /// Generate a container name for a project's install step.
    /// Separate from the dev container to avoid name collisions.
    #[inline]
    pub fn install_name(project_id: &str) -> String {
        format!("{NAME_PREFIX}install-{project_id}")
    }
}

/// Event name constants
pub mod events {
    /// Generate the log event name for a project
    #[inline]
    pub fn project_log(project_id: &str) -> String {
        format!("project-log-{project_id}")
    }

    /// Generate the sync event name for a project
    #[inline]
    pub fn project_sync(project_id: &str) -> String {
        format!("project-sync-{project_id}")
    }

    /// Generate the status event name for a project
    #[inline]
    pub fn project_status(project_id: &str) -> String {
        format!("project-status-{project_id}")
    }

    /// Generate the install step event name for a project
    #[inline]
    pub fn project_install_step(project_id: &str) -> String {
        format!("project-install-step-{project_id}")
    }

    /// Event name emitted when a store recovered from a corrupted state file.
    #[inline]
    pub fn state_recovery() -> &'static str {
        "state-recovery"
    }

    /// Event name for VM image download phase + byte counts. Payload shape:
    /// see `vm::image_downloader::VmImageProgress`.
    pub const VM_IMAGE_PROGRESS: &str = "vm-image-progress";
}

/// VM image distribution endpoint. The worker serves a manifest at this URL
/// pointing at the current image on R2. See `src/worker/src/index.ts` and
/// `docs/release-setup.md` section 7 for the server side.
#[cfg(target_os = "macos")]
pub const VM_IMAGE_MANIFEST_URL: &str = "https://openable.dev/vm/manifest.json";

/// Path constants and helpers
pub mod paths {
    use std::path::PathBuf;

    use crate::error::AppError;

    /// Base directory name under home
    pub const BASE_DIR: &str = ".dotapps";

    /// Repos subdirectory name
    pub const REPOS_DIR: &str = "repos";

    /// VM subdirectory name
    pub const VM_DIR: &str = "vm";

    /// State file name
    pub const STATE_FILE: &str = "state.json";

    /// Snapshots subdirectory name
    pub const SNAPSHOTS_DIR: &str = "snapshots";

    /// Logs subdirectory name
    pub const LOGS_DIR: &str = "logs";

    /// Read-only base VM image built by `make vm-image`
    pub const VM_BASE_IMAGE: &str = "alpine-base.img";

    /// Runtime APFS clone used by vfkit (disposable, recreated each startup)
    pub const VM_IMAGE: &str = "alpine.img";

    /// Get the base Opnble directory (~/.opnble)
    pub fn base_dir() -> Result<PathBuf, AppError> {
        dirs::home_dir()
            .map(|home| home.join(BASE_DIR))
            .ok_or(AppError::StorageFailed {
                reason: "cannot resolve home directory".to_string(),
            })
    }

    /// Get the repos directory (~/.opnble/repos)
    pub fn repos_dir() -> Result<PathBuf, AppError> {
        base_dir().map(|base| base.join(REPOS_DIR))
    }

    /// Get the VM directory (~/.opnble/vm)
    pub fn vm_dir() -> Result<PathBuf, AppError> {
        base_dir().map(|base| base.join(VM_DIR))
    }

    /// Host log directory (~/.opnble/logs). Rotated daily by tracing-appender.
    pub fn logs_dir() -> Result<PathBuf, AppError> {
        base_dir().map(|base| base.join(LOGS_DIR))
    }

    /// Get the state file path (~/.opnble/state.json)
    pub fn state_file() -> Result<PathBuf, AppError> {
        base_dir().map(|base| base.join(STATE_FILE))
    }

    /// Path to the read-only base VM image built by `make vm-image`
    pub fn vm_base_image() -> Result<PathBuf, AppError> {
        vm_dir().map(|vm| vm.join(VM_BASE_IMAGE))
    }

    /// JSON file recording which `imageVersion` is installed locally.
    /// Lives next to the base image so a `rm -rf ~/.opnble/vm/*` resets
    /// both the image and the version stamp atomically.
    pub fn vm_image_version_file() -> Result<PathBuf, AppError> {
        vm_dir().map(|vm| vm.join("image-version.json"))
    }

    /// Path to the runtime APFS clone used by vfkit (disposable, recreated each startup)
    pub fn vm_image() -> Result<PathBuf, AppError> {
        vm_dir().map(|vm| vm.join(VM_IMAGE))
    }

    /// Get the repository path for a specific project
    pub fn project_repo(project_id: &str) -> Result<PathBuf, AppError> {
        repos_dir().map(|repos| repos.join(project_id))
    }

    /// Get the snapshots directory (~/.opnble/snapshots)
    pub fn snapshots_dir() -> Result<PathBuf, AppError> {
        base_dir().map(|base| base.join(SNAPSHOTS_DIR))
    }

    /// Get the snapshot directory for a specific project + snapshot
    /// (`~/.opnble/snapshots/<project_id>/<snapshot_id>`)
    #[allow(
        dead_code,
        reason = "consumed in Task 7.1 (mcp_delete_snapshot Tauri command)"
    )]
    pub fn project_snapshot_dir(project_id: &str, snapshot_id: &str) -> Result<PathBuf, AppError> {
        snapshots_dir().map(|root| root.join(project_id).join(snapshot_id))
    }

    /// Path to the gvproxy API control socket
    pub fn gvproxy_api_socket() -> Result<PathBuf, AppError> {
        vm_dir().map(|vm| vm.join(super::vm::GVPROXY_SOCKET))
    }

    /// Path to the gvproxy-vfkit network socket
    pub fn gvproxy_vfkit_socket() -> Result<PathBuf, AppError> {
        vm_dir().map(|vm| vm.join(super::vm::GVPROXY_VFKIT_SOCKET))
    }
}

/// Port allocation constants
pub mod ports {
    /// Minimum port number for project allocation (default for `next_port`)
    pub const MIN_PORT: u16 = 3001;

    /// Default value for `next_port` when missing from state
    pub const DEFAULT_NEXT_PORT: u16 = MIN_PORT;

    /// Maximum port number for project allocation
    pub const MAX_PORT: u16 = 65000;
}

/// VM communication constants
pub mod vm {
    /// vsock port the guest agent listens on
    pub const AGENT_VSOCK_PORT: u32 = 1024;

    /// `VirtioFS` mount tag used inside the VM
    pub const VIRTIOFS_MOUNT_TAG: &str = "opnble-repos";

    /// Path where `VirtioFS` is mounted inside the VM
    pub const VIRTIOFS_GUEST_MOUNT: &str = "/repos";

    /// gvproxy control socket for port forwarding API
    pub const GVPROXY_SOCKET: &str = "gvproxy-api.sock";

    /// gvproxy-to-vfkit network socket
    pub const GVPROXY_VFKIT_SOCKET: &str = "gvproxy-vfkit.sock";
}

/// Install flow polling constants
pub mod install {
    use std::time::Duration;

    /// Interval between polling attempts during install.
    pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

    /// Maximum time to wait for VM to become reachable (60 seconds).
    pub const VM_READY_MAX_ATTEMPTS: u32 = 30;

    /// Maximum time to wait for HTTP server to respond (180 seconds).
    pub const HTTP_READY_MAX_ATTEMPTS: u32 = 90;
}

/// Cloudflare tunnel constants
pub mod tunnel {
    /// Base URL for the Opnble tunnel API
    pub const API_BASE_URL: &str = "https://tunnel.openable.dev";

    /// Binary name for the Cloudflare tunnel client
    pub const CLOUDFLARED_BINARY: &str = "cloudflared";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_name() {
        assert_eq!(containers::name("my-project"), "opnble-my-project");
        assert_eq!(containers::name("123"), "opnble-123");
    }

    #[test]
    fn test_project_log_event() {
        assert_eq!(events::project_log("abc"), "project-log-abc");
    }

    #[test]
    fn test_project_sync_event() {
        assert_eq!(events::project_sync("abc"), "project-sync-abc");
    }

    #[test]
    fn test_paths_are_consistent() {
        // Verify that path composition is consistent
        let base = paths::base_dir().unwrap();
        let repos = paths::repos_dir().unwrap();
        let vm = paths::vm_dir().unwrap();
        let state = paths::state_file().unwrap();

        assert!(repos.starts_with(&base));
        assert!(vm.starts_with(&base));
        assert!(state.starts_with(&base));

        assert_eq!(repos.file_name().unwrap(), paths::REPOS_DIR);
        assert_eq!(vm.file_name().unwrap(), paths::VM_DIR);
        assert_eq!(state.file_name().unwrap(), paths::STATE_FILE);
    }

    #[test]
    fn test_project_repo_path() {
        let path = paths::project_repo("test-project").unwrap();
        assert!(path.ends_with("test-project"));
        assert!(path.to_string_lossy().contains(paths::REPOS_DIR));
    }

    #[test]
    fn test_ports_default_next_port_consistency() {
        assert_eq!(ports::MIN_PORT, ports::DEFAULT_NEXT_PORT);
        assert_eq!(ports::DEFAULT_NEXT_PORT, 3001);
    }

    #[test]
    fn test_vm_base_image_path() {
        let path = paths::vm_base_image().unwrap();
        assert!(path.ends_with(paths::VM_BASE_IMAGE));
        assert!(path.to_string_lossy().contains(paths::VM_DIR));
    }

    #[test]
    fn test_vm_image_path() {
        let path = paths::vm_image().unwrap();
        assert!(path.ends_with(paths::VM_IMAGE));
        assert!(path.to_string_lossy().contains(paths::VM_DIR));
    }

    #[test]
    fn test_tunnel_constants() {
        assert!(tunnel::API_BASE_URL.starts_with("https://"));
        assert!(!tunnel::CLOUDFLARED_BINARY.is_empty());
    }

    #[test]
    fn install_cmd_generates_smart_entrypoint() {
        let cmd = containers::install_cmd();
        assert!(cmd.contains("if [ ! -d node_modules ]"));
        assert!(cmd.contains("package.json -nt node_modules/.package-lock.json"));
        assert!(cmd.contains("package-lock.json -nt node_modules/.package-lock.json"));
        assert!(cmd.contains("npm install"));
        assert!(cmd.contains("{ [ -f package-lock.json ]"));
        // Must NOT contain dev server startup
        assert!(!cmd.contains("npm run dev"));
    }

    #[test]
    fn snapshots_dir_is_under_base_dir() {
        let snapshots = paths::snapshots_dir().expect("snapshots_dir");
        let base = paths::base_dir().expect("base_dir");
        assert!(snapshots.starts_with(&base));
        assert_eq!(
            snapshots.file_name().and_then(|s| s.to_str()),
            Some("snapshots")
        );
    }

    #[test]
    fn dev_cmd_starts_server_only() {
        let cmd = containers::dev_cmd(3000);
        assert_eq!(cmd, "npm run dev -- --port 3000 --host");
        // Must NOT contain npm install logic
        assert!(!cmd.contains("node_modules"));
    }
}
