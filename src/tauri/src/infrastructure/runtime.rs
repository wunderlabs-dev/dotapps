//! Container runtime abstraction (Podman/Docker via bollard)

use std::path::Path;

use async_trait::async_trait;

use crate::error::AppError;

/// Result of a one-shot container run (build, lint, etc.)
#[derive(Clone, Debug)]
pub struct BuildResult {
    pub exit_code: i32,
    pub logs: String,
}

/// Trait for container runtime operations
#[async_trait]
pub trait Runtime: Send + Sync {
    /// Start a container for a project
    async fn start(
        &self,
        project_id: &str,
        repo_path: &Path,
        host_port: u16,
    ) -> Result<(), AppError>;

    /// Stop a container
    async fn stop(&self, container_id: &str) -> Result<(), AppError>;

    /// Remove a container
    async fn remove(&self, container_id: &str) -> Result<(), AppError>;

    /// Check if a container is running
    async fn is_running(&self, container_id: &str) -> bool;

    /// Ensure the required container image is available.
    ///
    /// Default is a no-op. Override on platforms that need to pull images
    /// (e.g., Linux Podman, WSL2).
    async fn ensure_image(&self) -> Result<(), AppError> {
        Ok(())
    }

    /// Whether the underlying runtime backend is reachable.
    ///
    /// On macOS/Windows this checks if the VM gateway is available.
    /// Native runtimes (Linux Podman) are always reachable.
    async fn is_reachable(&self) -> bool {
        true
    }

    /// Run a one-shot container with a custom command, wait for exit, return result.
    ///
    /// Used for builds: starts a temporary container with the given command,
    /// polls until exit, captures logs, cleans up the container.
    /// The `build_id` is used as the container name prefix (separate from project containers).
    async fn run_to_completion(
        &self,
        build_id: &str,
        repo_path: &Path,
        cmd: &str,
    ) -> Result<BuildResult, AppError> {
        let _ = (build_id, repo_path, cmd);
        Err(AppError::Internal {
            reason: "build not supported on this runtime".into(),
        })
    }

    /// Run npm install as a one-shot container. Returns Ok(()) if exit code is 0.
    ///
    /// Uses `run_to_completion` under the hood with an install-specific container name
    /// so it doesn't collide with the project's dev container.
    async fn run_install(&self, project_id: &str, repo_path: &Path) -> Result<(), AppError> {
        let install_id = crate::constants::containers::install_name(project_id);
        let cmd = crate::constants::containers::install_cmd();
        let result = self.run_to_completion(&install_id, repo_path, &cmd).await?;
        if result.exit_code != 0 {
            return Err(AppError::ContainerFailed {
                reason: format!(
                    "npm install failed (exit code {})\n{}",
                    result.exit_code,
                    result.logs.chars().take(500).collect::<String>()
                ),
            });
        }
        Ok(())
    }

    /// Return the last `lines` stdout/stderr lines from the project's container.
    ///
    /// The container engine is the source of truth: there is no app-side log buffer.
    /// Returns `AppError::NotFound` when the container is missing on platforms that
    /// can distinguish that case, and platform-specific errors otherwise.
    async fn tail_logs(&self, project_id: &str, lines: u32) -> Result<Vec<String>, AppError>;
}

/// Decode raw container log bytes into trimmed lines.
///
/// Concatenated bollard frame bytes or raw `podman logs` stdout share the same
/// shape: a UTF-8 byte stream where lines are separated by `\n` and may carry
/// trailing `\r` from CRLF terminals. The trailing empty element produced by a
/// stream that ends with `\n` is dropped so callers see only real log lines.
#[cfg(any(target_os = "linux", target_os = "windows", test))]
pub(crate) fn parse_log_frames(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines: Vec<String> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

/// Use as a fallback when the platform container runtime is unavailable.
///
/// Returns errors for all operations (null-object pattern).
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod stub {
    use std::path::Path;

    use async_trait::async_trait;

    use super::Runtime;
    use crate::error::AppError;

    pub struct StubRuntime;

    #[async_trait]
    impl Runtime for StubRuntime {
        async fn start(&self, _: &str, _: &Path, _: u16) -> Result<(), AppError> {
            Err(AppError::ContainerFailed {
                reason: "cannot start container: runtime not available".to_string(),
            })
        }

        async fn stop(&self, _: &str) -> Result<(), AppError> {
            Err(AppError::ContainerFailed {
                reason: "cannot stop container: runtime not available".to_string(),
            })
        }

        async fn remove(&self, _: &str) -> Result<(), AppError> {
            Err(AppError::ContainerFailed {
                reason: "cannot remove container: runtime not available".to_string(),
            })
        }

        async fn is_running(&self, _: &str) -> bool {
            false
        }

        async fn is_reachable(&self) -> bool {
            false
        }

        async fn tail_logs(&self, _: &str, _: u32) -> Result<Vec<String>, AppError> {
            Err(AppError::ContainerFailed {
                reason: "runtime unavailable".into(),
            })
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub use self::stub::StubRuntime;

/// Podman/Docker runtime implementation via bollard (Linux-only)
#[cfg(target_os = "linux")]
mod bollard_runtime {
    use std::collections::HashMap;
    use std::path::Path;

    use async_trait::async_trait;
    use bollard::container::{
        Config, CreateContainerOptions, LogOutput, LogsOptions, RemoveContainerOptions,
        StartContainerOptions, StopContainerOptions,
    };
    use bollard::image::CreateImageOptions;
    use bollard::models::{HostConfig, Mount, MountTypeEnum, PortBinding};
    use bollard::Docker;
    use futures::StreamExt;

    use super::{BuildResult, Runtime};
    use crate::constants::containers;
    use crate::error::AppError;

    /// Convert a bollard error into `AppError::ContainerFailed` with a verb-prefixed message.
    fn container_err(action: &str) -> impl FnOnce(bollard::errors::Error) -> AppError + '_ {
        move |e| AppError::ContainerFailed {
            reason: format!("cannot {action}: {e}"),
        }
    }

    pub struct PodmanRuntime {
        docker: Docker,
    }

    impl PodmanRuntime {
        pub fn new() -> Result<Self, AppError> {
            let docker = Docker::connect_with_socket_defaults()
                .map_err(container_err("connect to container runtime"))?;

            Ok(Self { docker })
        }
    }

    #[async_trait]
    impl Runtime for PodmanRuntime {
        async fn start(
            &self,
            project_id: &str,
            repo_path: &Path,
            host_port: u16,
        ) -> Result<(), AppError> {
            let container_name = containers::name(project_id);

            // Check if a container already exists for this project
            if let Ok(info) = self.docker.inspect_container(&container_name, None).await {
                let is_running = info.state.as_ref().and_then(|s| s.running).unwrap_or(false);

                if is_running {
                    return Ok(());
                }

                // Stopped container exists, restart it
                self.docker
                    .start_container(&container_name, None::<StartContainerOptions<String>>)
                    .await
                    .map_err(container_err("restart existing container"))?;

                return Ok(());
            }

            // No existing container: create fresh
            self.ensure_image().await?;

            // Determine which image to use
            let image = if self
                .docker
                .inspect_image(containers::BUNDLED_IMAGE)
                .await
                .is_ok()
            {
                containers::BUNDLED_IMAGE
            } else {
                containers::NODE_IMAGE
            };

            // Port mapping: host_port -> 3000 (default Next.js port)
            let mut port_bindings = HashMap::new();
            port_bindings.insert(
                format!("{}/tcp", containers::INTERNAL_PORT),
                Some(vec![PortBinding {
                    host_ip: Some("127.0.0.1".to_string()),
                    host_port: Some(host_port.to_string()),
                }]),
            );

            // Mount the repo into /app
            let mount = Mount {
                target: Some("/app".to_string()),
                source: Some(repo_path.to_string_lossy().to_string()),
                typ: Some(MountTypeEnum::BIND),
                read_only: Some(false),
                ..Default::default()
            };

            let host_config = HostConfig {
                port_bindings: Some(port_bindings),
                mounts: Some(vec![mount]),
                ..Default::default()
            };

            let config = Config {
                image: Some(image.to_string()),
                working_dir: Some("/app".to_string()),
                cmd: Some(vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    containers::dev_cmd(containers::INTERNAL_PORT),
                ]),
                exposed_ports: Some({
                    let mut ports = HashMap::new();
                    ports.insert(format!("{}/tcp", containers::INTERNAL_PORT), HashMap::new());
                    ports
                }),
                host_config: Some(host_config),
                tty: Some(true),
                ..Default::default()
            };

            let options = CreateContainerOptions {
                name: &container_name,
                platform: None,
            };

            self.docker
                .create_container(Some(options), config)
                .await
                .map_err(container_err("create container"))?;

            self.docker
                .start_container(&container_name, None::<StartContainerOptions<String>>)
                .await
                .map_err(container_err("start container"))?;

            Ok(())
        }

        async fn stop(&self, container_id: &str) -> Result<(), AppError> {
            self.docker
                .stop_container(container_id, Some(StopContainerOptions { t: 10 }))
                .await
                .map_err(container_err("stop container"))
        }

        async fn remove(&self, container_id: &str) -> Result<(), AppError> {
            // Stop first if running
            let _ = self.stop(container_id).await;

            self.docker
                .remove_container(
                    container_id,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await
                .map_err(container_err("remove container"))
        }

        async fn is_running(&self, container_id: &str) -> bool {
            match self.docker.inspect_container(container_id, None).await {
                Ok(info) => info.state.and_then(|s| s.running).unwrap_or(false),
                Err(_) => false,
            }
        }

        async fn ensure_image(&self) -> Result<(), AppError> {
            // First try bundled image
            if self
                .docker
                .inspect_image(containers::BUNDLED_IMAGE)
                .await
                .is_ok()
            {
                return Ok(());
            }

            // Fall back to upstream Node image
            if self
                .docker
                .inspect_image(containers::NODE_IMAGE)
                .await
                .is_ok()
            {
                return Ok(());
            }

            // Pull upstream as fallback
            let options = CreateImageOptions {
                from_image: containers::NODE_IMAGE,
                ..Default::default()
            };

            let mut stream = self.docker.create_image(Some(options), None, None);

            while let Some(result) = stream.next().await {
                if let Err(e) = result {
                    return Err(container_err("pull image")(e));
                }
            }

            Ok(())
        }

        async fn run_to_completion(
            &self,
            build_id: &str,
            repo_path: &Path,
            cmd: &str,
        ) -> Result<BuildResult, AppError> {
            use bollard::container::WaitContainerOptions;

            let container_name = containers::name(build_id);

            // Remove stale build container if one exists
            let _ = self
                .docker
                .remove_container(
                    &container_name,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await;

            // Mount the repo at /app, run the build command
            let mount = Mount {
                target: Some("/app".to_string()),
                source: Some(repo_path.to_string_lossy().to_string()),
                typ: Some(MountTypeEnum::BIND),
                read_only: Some(false),
                ..Default::default()
            };

            let config = Config {
                image: Some(containers::NODE_IMAGE.to_string()),
                cmd: Some(vec!["sh".to_string(), "-c".to_string(), cmd.to_string()]),
                working_dir: Some("/app".to_string()),
                host_config: Some(HostConfig {
                    mounts: Some(vec![mount]),
                    ..Default::default()
                }),
                ..Default::default()
            };

            self.docker
                .create_container(
                    Some(CreateContainerOptions {
                        name: container_name.as_str(),
                        platform: None,
                    }),
                    config,
                )
                .await
                .map_err(container_err("create build container"))?;

            self.docker
                .start_container(&container_name, None::<StartContainerOptions<String>>)
                .await
                .map_err(container_err("start build container"))?;

            // Wait for the container to exit
            let mut wait_stream = self.docker.wait_container(
                &container_name,
                Some(WaitContainerOptions {
                    condition: "not-running",
                }),
            );
            let exit_code = match wait_stream.next().await {
                Some(Ok(response)) => i32::try_from(response.status_code).unwrap_or(-1),
                Some(Err(e)) => {
                    tracing::warn!("cannot wait for build container: {e}");
                    -1
                }
                None => -1,
            };

            // Capture logs
            let log_opts = LogsOptions::<String> {
                stdout: true,
                stderr: true,
                tail: "200".to_string(),
                ..Default::default()
            };
            let mut log_stream = self.docker.logs(&container_name, Some(log_opts));
            let mut logs = String::new();
            while let Some(Ok(output)) = log_stream.next().await {
                match output {
                    LogOutput::StdOut { message } | LogOutput::StdErr { message } => {
                        logs.push_str(&String::from_utf8_lossy(&message));
                    }
                    LogOutput::Console { message } => {
                        logs.push_str(&String::from_utf8_lossy(&message));
                    }
                    LogOutput::StdIn { .. } => {}
                }
            }

            // Clean up
            let _ = self
                .docker
                .remove_container(
                    &container_name,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await;

            Ok(BuildResult { exit_code, logs })
        }

        async fn tail_logs(&self, project_id: &str, lines: u32) -> Result<Vec<String>, AppError> {
            let container_name = containers::name(project_id);
            let opts = LogsOptions::<String> {
                follow: false,
                stdout: true,
                stderr: true,
                tail: lines.to_string(),
                timestamps: false,
                ..Default::default()
            };

            let mut stream = self.docker.logs(&container_name, Some(opts));
            let mut buffer: Vec<u8> = Vec::new();
            while let Some(item) = stream.next().await {
                match item {
                    Ok(frame) => buffer.extend_from_slice(&frame.into_bytes()),
                    Err(err) => {
                        if let bollard::errors::Error::DockerResponseServerError {
                            status_code: 404,
                            ..
                        } = &err
                        {
                            return Err(AppError::NotFound {
                                entity: "container".into(),
                                id: project_id.to_string(),
                            });
                        }
                        return Err(container_err("read container logs")(err));
                    }
                }
            }

            Ok(super::parse_log_frames(&buffer))
        }
    }
}

#[cfg(target_os = "linux")]
pub use bollard_runtime::PodmanRuntime;

#[cfg(test)]
#[expect(
    clippy::panic,
    reason = "test-only refutable-let-else branches must panic to fail the test"
)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_trait_is_object_safe() {
        // Verify trait can be used with dyn
        fn _accepts_dyn(_: &dyn Runtime) {}
    }

    #[test]
    fn parse_log_frames_returns_empty_for_empty_input() {
        assert!(parse_log_frames(b"").is_empty());
    }

    #[test]
    fn parse_log_frames_drops_trailing_empty_line_after_newline() {
        let lines = parse_log_frames(b"hello world\n");
        assert_eq!(lines, vec!["hello world".to_string()]);
    }

    #[test]
    fn parse_log_frames_keeps_partial_line_without_trailing_newline() {
        let lines = parse_log_frames(b"hello\nworld");
        assert_eq!(lines, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn parse_log_frames_strips_trailing_carriage_returns() {
        let lines = parse_log_frames(b"first\r\nsecond\r\nthird\r\n");
        assert_eq!(
            lines,
            vec![
                "first".to_string(),
                "second".to_string(),
                "third".to_string()
            ]
        );
    }

    #[test]
    fn parse_log_frames_preserves_blank_lines_in_middle() {
        let lines = parse_log_frames(b"a\n\nb\n");
        assert_eq!(lines, vec!["a".to_string(), String::new(), "b".to_string()]);
    }

    #[test]
    fn parse_log_frames_handles_concatenated_multiplexed_frames() {
        // Simulate two bollard frames (StdOut then StdErr) concatenated by the caller.
        let mut buffer = Vec::new();
        buffer.extend_from_slice(b"compiling app\n");
        buffer.extend_from_slice(b"warning: unused import\n");
        let lines = parse_log_frames(&buffer);
        assert_eq!(
            lines,
            vec![
                "compiling app".to_string(),
                "warning: unused import".to_string()
            ]
        );
    }

    #[test]
    fn parse_log_frames_replaces_invalid_utf8_lossily() {
        // 0xff is invalid UTF-8; from_utf8_lossy substitutes U+FFFD.
        let bytes = [b'a', 0xff, b'\n', b'b', b'\n'];
        let lines = parse_log_frames(&bytes);
        let [first, second] = lines.as_slice() else {
            panic!("expected exactly two lines, got {lines:?}");
        };
        assert!(first.contains('a'));
        assert!(first.contains('\u{FFFD}'));
        assert_eq!(second, "b");
    }
}
