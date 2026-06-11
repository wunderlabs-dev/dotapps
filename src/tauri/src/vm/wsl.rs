//! WSL2 integration for Windows

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::shell::shell_escape;
use super::VmError;
use crate::constants::containers;
use crate::error::AppError;
use crate::infrastructure::runtime::Runtime;

const WSL_DISTRO_NAME: &str = "opnble-linux";

#[derive(Clone)]
pub struct WSL2 {
    distro_name: String,
    initialized: bool,
}

impl WSL2 {
    pub fn new() -> Self {
        Self {
            distro_name: WSL_DISTRO_NAME.to_string(),
            initialized: false,
        }
    }

    /// Check if WSL2 is available and enabled
    pub fn is_available() -> bool {
        let output = Command::new("wsl").args(["--status"]).output();

        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }

    /// Check if our distro is installed
    pub fn is_distro_installed(&self) -> bool {
        let output = Command::new("wsl").args(["-l", "-q"]).output();

        match output {
            Ok(o) => {
                let list = String::from_utf8_lossy(&o.stdout);
                list.lines().any(|l| l.trim() == self.distro_name)
            }
            Err(_) => false,
        }
    }

    /// Install our custom distro from a tar file
    pub fn install_distro(&mut self, tar_path: &Path, install_dir: &Path) -> Result<(), VmError> {
        // Create install directory
        std::fs::create_dir_all(install_dir)?;

        let output = Command::new("wsl")
            .args([
                "--import",
                &self.distro_name,
                &install_dir.to_string_lossy(),
                &tar_path.to_string_lossy(),
                "--version",
                "2",
            ])
            .output()
            .map_err(|e| VmError::SetupFailed {
                reason: format!("cannot import distro: {e}"),
            })?;

        if !output.status.success() {
            return Err(VmError::SetupFailed {
                reason: format!(
                    "cannot import distro: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        self.initialized = true;
        Ok(())
    }

    /// Run a command in the WSL distro (internal, pre-escaped).
    ///
    /// The command is passed as a pre-escaped string. For executing programs
    /// with arguments, use `run_args` instead which handles escaping.
    fn run_command_raw(&self, command: &str) -> Result<String, VmError> {
        let output = Command::new("wsl")
            .args(["-d", &self.distro_name, "--", "sh", "-c", command])
            .output()
            .map_err(|e| VmError::CommandFailed {
                reason: format!("cannot execute wsl command: {e}"),
            })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(VmError::CommandFailed {
                reason: format!(
                    "cannot run wsl command: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            })
        }
    }

    /// Run a program with arguments in the WSL distro.
    ///
    /// Each argument is shell-escaped to prevent command injection.
    /// The program name and all arguments are safely quoted.
    pub fn run_args(&self, program: &str, args: &[&str]) -> Result<String, VmError> {
        let escaped_parts: Vec<String> = std::iter::once(program)
            .chain(args.iter().copied())
            .map(shell_escape)
            .collect();
        let command = escaped_parts.join(" ");
        self.run_command_raw(&command)
    }

    /// Run Podman command in WSL
    pub fn podman(&self, args: &[&str]) -> Result<String, VmError> {
        self.run_args("podman", args)
    }

    /// Run a simple command with no arguments in WSL.
    ///
    /// For commands with arguments, use `run_args` instead.
    /// This method shell-escapes the command to prevent injection.
    pub fn run(&self, command: &str) -> Result<String, VmError> {
        // For backward compatibility with simple commands like "podman --version"
        // Split on whitespace and escape each part
        let parts: Vec<&str> = command.split_whitespace().collect();
        let (&program, args) = parts.split_first().ok_or_else(|| VmError::CommandFailed {
            reason: "cannot execute empty command".to_string(),
        })?;
        self.run_args(program, args)
    }

    /// Start a container, reusing an existing stopped one when possible.
    pub fn start_container(
        &self,
        project_id: &str,
        repo_path: &str,
        port: u16,
    ) -> Result<String, VmError> {
        let container_name = containers::name(project_id);

        // Convert Windows path to WSL path
        let wsl_repo_path = self.normalize_repo_path(repo_path)?;

        // Check if container already exists
        if let Ok(output) =
            self.podman(&["inspect", "--format", "{{.State.Status}}", &container_name])
        {
            let status = output.trim();
            match status {
                "running" => return Ok(container_name),
                "exited" | "created" | "dead" => {
                    // Try to restart existing container
                    match self.podman(&["start", &container_name]) {
                        Ok(_) => return Ok(container_name),
                        Err(e) => {
                            // Restart failed, remove and fall through to create fresh
                            tracing::warn!("cannot restart existing container, will recreate: {e}");
                            let _ = self.podman(&["rm", "-f", &container_name]);
                        }
                    }
                }
                _ => {
                    // Unknown state, remove and recreate
                    let _ = self.podman(&["stop", &container_name]);
                    let _ = self.podman(&["rm", "-f", &container_name]);
                }
            }
        }

        // No existing container (or removed): create fresh
        self.podman(&[
            "run",
            "-d",
            "--name",
            &container_name,
            "-v",
            &format!("{wsl_repo_path}:/app"),
            "-w",
            "/app",
            "-p",
            &format!("{port}:{}", containers::INTERNAL_PORT),
            containers::NODE_IMAGE,
            "sh",
            "-c",
            &containers::dev_cmd(containers::INTERNAL_PORT),
        ])
    }

    /// Stop a container
    pub fn stop_container(&self, project_id: &str) -> Result<(), VmError> {
        let container_name = containers::name(project_id);
        self.podman(&["stop", &container_name])?;
        Ok(())
    }

    /// Remove a container
    pub fn remove_container(&self, project_id: &str) -> Result<(), VmError> {
        let container_name = containers::name(project_id);
        self.podman(&["rm", "-f", &container_name])?;
        Ok(())
    }

    /// Check if a container is running
    pub fn is_container_running(&self, project_id: &str) -> Result<bool, VmError> {
        let container_name = containers::name(project_id);
        let output =
            self.podman(&["inspect", "--format", "{{.State.Running}}", &container_name])?;
        Ok(output.trim() == "true")
    }

    /// Get the last `lines` log lines for a container as a single buffer.
    pub fn logs(&self, project_id: &str, lines: u32) -> Result<String, VmError> {
        let container_name = containers::name(project_id);
        let tail = lines.to_string();
        self.podman(&["logs", "--tail", &tail, &container_name])
    }

    /// Convert Windows path to WSL path
    fn windows_to_wsl_path(&self, windows_path: &str) -> Result<String, VmError> {
        // C:\Users\... -> /mnt/c/Users/...
        let output = Command::new("wsl")
            .args(["-d", &self.distro_name, "wslpath", "-u", windows_path])
            .output()
            .map_err(|e| VmError::Io {
                reason: format!("cannot convert path: {e}"),
            })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(VmError::Io {
                reason: "cannot convert windows path to wsl path".to_string(),
            })
        }
    }

    /// Normalize a host repo path into a WSL mount path used by container commands.
    pub fn normalize_repo_path(&self, host_path: &str) -> Result<String, VmError> {
        self.windows_to_wsl_path(host_path)
    }
}

impl Default for WSL2 {
    fn default() -> Self {
        Self::new()
    }
}

/// Runtime implementation that delegates container operations to WSL2 Podman.
pub struct WSL2Runtime {
    wsl: Arc<Mutex<Option<WSL2>>>,
}

impl WSL2Runtime {
    pub fn new(wsl: Arc<Mutex<Option<WSL2>>>) -> Self {
        Self { wsl }
    }

    fn wsl_not_initialized_error() -> AppError {
        AppError::ContainerFailed {
            reason: "cannot use wsl2: not initialized, run init_wsl first".to_string(),
        }
    }

    async fn current_wsl(&self) -> Result<WSL2, AppError> {
        let guard = self.wsl.lock().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(Self::wsl_not_initialized_error)
    }

    fn map_wsl_err(err: VmError) -> AppError {
        AppError::ContainerFailed {
            reason: err.to_string(),
        }
    }
}

#[async_trait]
impl Runtime for WSL2Runtime {
    async fn start(
        &self,
        project_id: &str,
        repo_path: &std::path::Path,
        host_port: u16,
    ) -> Result<(), AppError> {
        if !repo_path.exists() || !repo_path.is_dir() {
            return Err(AppError::ContainerFailed {
                reason: format!(
                    "cannot resolve repository path for wsl2 runtime: {}",
                    repo_path.display()
                ),
            });
        }

        let wsl = self.current_wsl().await?;
        let project_id_owned = project_id.to_string();
        let repo_path_owned = repo_path.to_string_lossy().to_string();

        tokio::task::spawn_blocking(move || {
            wsl.start_container(&project_id_owned, &repo_path_owned, host_port)
        })
        .await?
        .map_err(Self::map_wsl_err)?;

        Ok(())
    }

    async fn stop(&self, container_id: &str) -> Result<(), AppError> {
        let wsl = self.current_wsl().await?;
        let container_id_owned = container_id.to_string();
        tokio::task::spawn_blocking(move || wsl.stop_container(&container_id_owned))
            .await?
            .map_err(Self::map_wsl_err)?;
        Ok(())
    }

    async fn remove(&self, container_id: &str) -> Result<(), AppError> {
        let wsl = self.current_wsl().await?;
        let container_id_owned = container_id.to_string();
        tokio::task::spawn_blocking(move || wsl.remove_container(&container_id_owned))
            .await?
            .map_err(Self::map_wsl_err)?;
        Ok(())
    }

    async fn is_running(&self, container_id: &str) -> bool {
        let wsl = match self.current_wsl().await {
            Ok(wsl) => wsl,
            Err(_) => return false,
        };
        let container_id_owned = container_id.to_string();
        tokio::task::spawn_blocking(move || wsl.is_container_running(&container_id_owned))
            .await
            .ok()
            .and_then(|result| result.ok())
            .unwrap_or(false)
    }

    async fn ensure_image(&self) -> Result<(), AppError> {
        let wsl = self.current_wsl().await?;
        tokio::task::spawn_blocking(move || {
            if wsl
                .podman(&["image", "inspect", containers::NODE_IMAGE])
                .is_ok()
            {
                return Ok(());
            }
            wsl.podman(&["pull", containers::NODE_IMAGE]).map(|_| ())
        })
        .await?
        .map_err(Self::map_wsl_err)
    }

    async fn is_reachable(&self) -> bool {
        self.wsl.lock().await.is_some()
    }

    async fn tail_logs(&self, project_id: &str, lines: u32) -> Result<Vec<String>, AppError> {
        let wsl = self.current_wsl().await?;
        let project_id_owned = project_id.to_string();
        let raw = tokio::task::spawn_blocking(move || wsl.logs(&project_id_owned, lines))
            .await?
            .map_err(Self::map_wsl_err)?;
        Ok(crate::infrastructure::runtime::parse_log_frames(
            raw.as_bytes(),
        ))
    }
}
