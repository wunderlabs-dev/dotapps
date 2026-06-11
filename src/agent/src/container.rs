use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use bollard::container::{
    Config, CreateContainerOptions, ListContainersOptions, LogOutput, LogsOptions,
    RemoveContainerOptions, StopContainerOptions,
};
use bollard::models::{HostConfig, PortBinding};
use bollard::Docker;
use futures_util::StreamExt;

use crate::error::AgentError;

const CONTAINER_PREFIX: &str = "opnble-";

/// Default podman socket path (rootful, Linux).
pub const PODMAN_SOCKET: &str = "/run/podman/podman.sock";

/// Validate that a `project_id` contains only safe characters for use in
/// container names and shell arguments: `[a-zA-Z0-9._-]`, non-empty.
/// I8: Exposed for RPC boundary validation (defense-in-depth).
pub fn validate_project_id(project_id: &str) -> Result<(), String> {
    if project_id.is_empty() {
        return Err("project_id must not be empty".to_string());
    }
    if !project_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(format!(
            "project_id contains invalid characters: {project_id}"
        ));
    }
    Ok(())
}

pub fn container_name(project_id: &str) -> Result<String, String> {
    validate_project_id(project_id)?;
    Ok(format!("{CONTAINER_PREFIX}{project_id}"))
}

/// Opaque wrapper around bollard's `Docker` client.
///
/// Keeping the `bollard::Docker` value private behind this type prevents the
/// agent's public API from leaking the third-party transport (M-DONT-LEAK-TYPES).
/// All container lifecycle operations go through methods on `&PodmanRuntime`.
pub struct PodmanRuntime {
    docker: Docker,
}

impl PodmanRuntime {
    /// Connect to the local podman socket.
    ///
    /// Podman exposes a Docker-compatible REST API. On rootful Linux (the VM),
    /// the socket lives at `/run/podman/podman.sock`.
    pub fn connect() -> Result<Self, AgentError> {
        Docker::connect_with_unix(PODMAN_SOCKET, 120, bollard::API_DEFAULT_VERSION)
            .map(|docker| Self { docker })
            .map_err(|source| AgentError::PodmanConnect {
                socket: PathBuf::from(PODMAN_SOCKET),
                source,
            })
    }

    /// Crate-private accessor so sibling modules (e.g. `health`) can issue
    /// bollard calls without re-exposing `Docker` in the public API.
    pub(crate) fn docker(&self) -> &Docker {
        &self.docker
    }

    pub async fn start_container(
        &self,
        project_id: &str,
        repo_path: &str,
        port: u16,
        image: &str,
        cmd: &str,
        internal_port: u16,
    ) -> Result<String, AgentError> {
        let name = container_name(project_id).map_err(AgentError::other)?;

        if let Some(reused_id) = reuse_or_remove_existing_container(&self.docker, &name).await {
            return Ok(reused_id);
        }

        validate_repo_mount(repo_path).map_err(AgentError::other)?;

        let (port_bindings, exposed_ports) = build_port_bindings(port, internal_port);
        let config = build_container_config(image, cmd, repo_path, port_bindings, exposed_ports);
        create_and_start(&self.docker, name, config).await
    }

    pub async fn stop_container(&self, project_id: &str) -> Result<(), AgentError> {
        let name = container_name(project_id).map_err(AgentError::other)?;
        self.docker
            .stop_container(&name, Some(StopContainerOptions { t: 10 }))
            .await
            .map_err(|source| AgentError::ContainerStop { id: name, source })
    }

    pub async fn remove_container(&self, project_id: &str) -> Result<(), AgentError> {
        let name = container_name(project_id).map_err(AgentError::other)?;
        let _ = self
            .docker
            .stop_container(&name, None::<StopContainerOptions>)
            .await;
        self.docker
            .remove_container(
                &name,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(|source| AgentError::ContainerRemove { id: name, source })
    }

    pub async fn container_status(
        &self,
        project_id: &str,
    ) -> Result<(String, String, String), AgentError> {
        let name = container_name(project_id).map_err(AgentError::other)?;
        let resp = self
            .docker
            .inspect_container(&name, None)
            .await
            .map_err(|source| AgentError::ContainerInspect {
                id: name.clone(),
                source,
            })?;

        let id = resp.id.unwrap_or_default();
        let status = resp
            .state
            .and_then(|s| s.status)
            .map_or_else(|| "unknown".to_string(), |s| s.to_string());

        Ok((id, name, status))
    }

    /// List opnble containers. I7: Podman's name filter matches substring;
    /// we apply strict prefix filter so only names starting with "opnble-" are returned.
    pub async fn list_containers(&self) -> Result<Vec<(String, String, String)>, AgentError> {
        let mut filters = HashMap::new();
        filters.insert("name", vec![CONTAINER_PREFIX]);

        let options = ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        };

        let containers = self
            .docker
            .list_containers(Some(options))
            .await
            .map_err(|source| AgentError::ContainerList { source })?;

        Ok(containers
            .into_iter()
            .filter_map(|c| {
                let id = c.id?;
                // Docker/Podman API returns names prefixed with "/"
                let name = c
                    .names?
                    .into_iter()
                    .next()?
                    .trim_start_matches('/')
                    .to_string();
                let state = c.state.unwrap_or_default();

                // I7: Strict prefix - container name must start with "opnble-"
                if name.starts_with(CONTAINER_PREFIX) {
                    Some((id, name, state))
                } else {
                    None
                }
            })
            .collect())
    }

    /// Get recent log lines from a container without following.
    pub async fn recent_logs(
        &self,
        container_name: &str,
        tail: usize,
    ) -> Result<Vec<LogLine>, AgentError> {
        let options = LogsOptions::<String> {
            follow: false,
            stdout: true,
            stderr: true,
            tail: tail.to_string(),
            timestamps: false,
            ..Default::default()
        };

        let mut stream = self.docker.logs(container_name, Some(options));
        let mut entries = Vec::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(LogOutput::StdOut { message }) => {
                    entries.push(LogLine {
                        line: String::from_utf8_lossy(&message).trim_end().to_string(),
                        stream: "stdout".to_string(),
                        timestamp: String::new(),
                    });
                }
                Ok(LogOutput::StdErr { message }) => {
                    entries.push(LogLine {
                        line: String::from_utf8_lossy(&message).trim_end().to_string(),
                        stream: "stderr".to_string(),
                        timestamp: String::new(),
                    });
                }
                Ok(LogOutput::StdIn { .. } | LogOutput::Console { .. }) => {}
                Err(source) => {
                    return Err(AgentError::LogStream {
                        id: container_name.to_string(),
                        source,
                    });
                }
            }
        }

        Ok(entries)
    }
}

/// Reuse a healthy existing container or remove a broken one in preparation
/// for a fresh create.
///
/// Returns `Some(id)` when the existing container was running or could be
/// restarted, in which case the caller should return that id directly.
/// Returns `None` when no container existed, or when an existing container
/// was removed and the caller should proceed to create a fresh one.
async fn reuse_or_remove_existing_container(docker: &Docker, name: &str) -> Option<String> {
    let info = docker.inspect_container(name, None).await.ok()?;
    let status = info
        .state
        .as_ref()
        .and_then(|s| s.status)
        .map_or_else(|| "unknown".to_string(), |s| s.to_string());

    match status.as_str() {
        "running" => Some(info.id.unwrap_or_default()),
        "exited" | "created" | "dead" => match docker.start_container::<String>(name, None).await {
            Ok(()) => Some(info.id.unwrap_or_default()),
            Err(e) => {
                log::warn!("cannot restart existing container, will recreate: {e}");
                force_remove(docker, name).await;
                None
            }
        },
        _ => {
            let _ = docker
                .stop_container(name, None::<StopContainerOptions>)
                .await;
            force_remove(docker, name).await;
            None
        }
    }
}

async fn force_remove(docker: &Docker, name: &str) {
    let _ = docker
        .remove_container(
            name,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
}

fn validate_repo_mount(repo_path: &str) -> Result<(), String> {
    let mount_path = std::path::Path::new(repo_path);
    if !mount_path.exists() {
        return Err(format!(
            "repo mount path does not exist: {}",
            mount_path.display()
        ));
    }
    if !mount_path.is_dir() {
        return Err(format!(
            "repo mount path is not a directory: {}",
            mount_path.display()
        ));
    }
    Ok(())
}

type PortBindingsMap = HashMap<String, Option<Vec<PortBinding>>>;
#[expect(
    clippy::zero_sized_map_values,
    reason = "bollard API requires HashMap<(), ()> for ExposedPorts"
)]
type ExposedPortsMap = HashMap<String, HashMap<(), ()>>;

/// Build the host-port bindings and exposed-ports map for the container.
///
/// `port == 0` means no host-side forwarding (used for one-shot build
/// containers); both maps are then `None`.
#[expect(
    clippy::zero_sized_map_values,
    reason = "bollard API requires HashMap<(), ()> for ExposedPorts"
)]
fn build_port_bindings(
    port: u16,
    internal_port: u16,
) -> (Option<PortBindingsMap>, Option<ExposedPortsMap>) {
    if port == 0 {
        return (None, None);
    }
    let port_key = format!("{internal_port}/tcp");
    let mut bindings: PortBindingsMap = HashMap::new();
    bindings.insert(
        port_key.clone(),
        Some(vec![PortBinding {
            host_ip: Some("0.0.0.0".to_string()),
            host_port: Some(port.to_string()),
        }]),
    );

    let mut exposed: ExposedPortsMap = HashMap::new();
    exposed.insert(port_key, HashMap::new());

    (Some(bindings), Some(exposed))
}

fn build_container_config(
    image: &str,
    cmd: &str,
    repo_path: &str,
    port_bindings: Option<PortBindingsMap>,
    exposed_ports: Option<ExposedPortsMap>,
) -> Config<String> {
    // Host networking: containers share the VM's network stack so they
    // can reach the internet via gvproxy (192.168.127.1). Podman's default
    // bridge (10.88.0.0/16) has no route to the VM's host network.
    let host_config = HostConfig {
        binds: Some(vec![format!("{repo_path}:/app")]),
        port_bindings,
        network_mode: Some("host".to_string()),
        ..Default::default()
    };

    Config {
        image: Some(image.to_string()),
        cmd: Some(vec!["sh".to_string(), "-c".to_string(), cmd.to_string()]),
        working_dir: Some("/app".to_string()),
        exposed_ports,
        host_config: Some(host_config),
        env: Some(vec![
            // Disable hostname validation for dev servers accessed via tunnel.
            "DANGEROUSLY_DISABLE_HOST_CHECK=true".to_string(),
        ]),
        ..Default::default()
    }
}

async fn create_and_start(
    docker: &Docker,
    name: String,
    config: Config<String>,
) -> Result<String, AgentError> {
    let create_name = name.clone();
    let options = CreateContainerOptions {
        name,
        ..Default::default()
    };
    let response = docker
        .create_container(Some(options), config)
        .await
        .map_err(|source| AgentError::ContainerStart {
            name: create_name.clone(),
            source,
        })?;
    docker
        .start_container::<String>(&response.id, None)
        .await
        .map_err(|source| AgentError::ContainerStart {
            name: create_name,
            source,
        })?;
    Ok(response.id)
}

/// A single log line with its stream origin.
pub struct LogLine {
    pub line: String,
    pub stream: String, // "stdout" or "stderr"
    pub timestamp: String,
}

/// I6: Use /proc/stat and /proc/meminfo for robust parsing (avoids `BusyBox` top format differences).
/// Falls back to shell commands if /proc is unavailable (e.g. non-Linux).
pub fn stats() -> Result<(f32, u64, u64), String> {
    if let Ok((cpu, used, total)) = stats_proc() {
        return Ok((cpu, used, total));
    }
    stats_fallback()
}

fn stats_proc() -> Result<(f32, u64, u64), String> {
    let cpu = parse_cpu_proc_stat("/proc/stat")?;
    let (used, total) = parse_mem_proc_meminfo("/proc/meminfo")?;
    Ok((cpu, used, total))
}

fn parse_cpu_proc_stat(path: &str) -> Result<f32, String> {
    let stat = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    let first_line = stat
        .lines()
        .next()
        .ok_or_else(|| "empty /proc/stat".to_string())?;
    if !first_line.starts_with("cpu ") {
        return Err(format!("unexpected /proc/stat format: {first_line}"));
    }
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    // cpu user nice system idle iowait irq softirq steal guest guest_nice
    if parts.len() < 5 {
        return Err(format!("insufficient fields in /proc/stat: {first_line}"));
    }
    let user: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let nice: u64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let system: u64 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let idle: u64 = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
    let iowait: u64 = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
    let irq: u64 = parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0);
    let softirq: u64 = parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0);
    let steal: u64 = parts.get(8).and_then(|s| s.parse().ok()).unwrap_or(0);

    let total = user + nice + system + idle + iowait + irq + softirq + steal;
    let busy = total.saturating_sub(idle);
    #[expect(
        clippy::cast_precision_loss,
        clippy::as_conversions,
        reason = "CPU stats don't need u64 precision, no TryFrom for u64 -> f32"
    )]
    let cpu_percent = if total > 0 {
        (busy as f32 / total as f32) * 100.0
    } else {
        0.0
    };
    Ok(cpu_percent.min(100.0))
}

fn parse_mem_proc_meminfo(path: &str) -> Result<(u64, u64), String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    let mut mem_total_kb: u64 = 0;
    let mut mem_available_kb: u64 = 0;
    let mut mem_free_kb: u64 = 0;
    let mut buffers_kb: u64 = 0;
    let mut cached_kb: u64 = 0;

    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            mem_total_kb = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        } else if line.starts_with("MemAvailable:") {
            mem_available_kb = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        } else if line.starts_with("MemFree:") {
            mem_free_kb = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        } else if line.starts_with("Buffers:") {
            buffers_kb = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        } else if line.starts_with("Cached:") {
            cached_kb = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        }
    }

    if mem_total_kb == 0 {
        return Err("MemTotal not found in /proc/meminfo".to_string());
    }

    // MemAvailable (kernel 3.14+) or fallback: MemFree + Buffers + Cached
    let available = if mem_available_kb > 0 {
        mem_available_kb
    } else {
        mem_free_kb
            .saturating_add(buffers_kb)
            .saturating_add(cached_kb)
    };

    let consumed_kb = mem_total_kb.saturating_sub(available);
    let used_mb = consumed_kb / 1024;
    let total_mb = mem_total_kb / 1024;

    Ok((used_mb, total_mb))
}

/// Disk space for the root filesystem via `df -m /`.
/// Returns (`available_mb`, `total_mb`).
pub fn disk_stats() -> Result<(u64, u64), String> {
    let output = Command::new("df")
        .args(["-m", "/"])
        .output()
        .map_err(|e| format!("cannot run df: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "cannot collect disk stats: df exited with {}: {stderr}",
            output.status
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // df -m output: Filesystem 1M-blocks Used Available Use% Mounted on
    //               /dev/vda2      3950  850     2876  23% /
    let line = stdout
        .lines()
        .nth(1)
        .ok_or_else(|| "cannot parse df output: missing data line".to_string())?;
    let cols: Vec<&str> = line.split_whitespace().collect();
    if cols.len() < 4 {
        return Err(format!("unexpected df output: {line}"));
    }
    let total: u64 = cols
        .get(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("cannot parse total from df: {line}"))?;
    let available: u64 = cols
        .get(3)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("cannot parse available from df: {line}"))?;

    Ok((available, total))
}

fn stats_fallback() -> Result<(f32, u64, u64), String> {
    let cpu_output = Command::new("sh")
        .args([
            "-c",
            "top -bn1 2>/dev/null | grep -E '^CPU:' | awk '{print $8}' | tr -d '%' || echo 0",
        ])
        .output()
        .map_err(|e| e.to_string())?;

    let cpu_idle_raw = String::from_utf8_lossy(&cpu_output.stdout);
    let cpu_idle_str = cpu_idle_raw.trim();
    let cpu_idle: f32 = cpu_idle_str.parse().unwrap_or_else(|e| {
        log::warn!("cannot parse cpu idle '{cpu_idle_str}': {e}; using fallback 100.0");
        100.0
    });
    let cpu_percent = (100.0 - cpu_idle).clamp(0.0, 100.0);

    let mem_output = Command::new("sh")
        .args([
            "-c",
            "free -m 2>/dev/null | awk 'NR==2{printf \"%s %s\", $3, $2}' || echo '0 2048'",
        ])
        .output()
        .map_err(|e| e.to_string())?;

    let mem_str = String::from_utf8_lossy(&mem_output.stdout);
    let parts: Vec<&str> = mem_str.split_whitespace().collect();
    let used: u64 = if let Some(s) = parts.first() {
        s.parse().unwrap_or_else(|e| {
            log::warn!("cannot parse memory used '{s}': {e}; using fallback 0");
            0
        })
    } else {
        log::warn!("memory stats output missing used field; using fallback 0");
        0
    };
    let total: u64 = if let Some(s) = parts.get(1) {
        s.parse().unwrap_or_else(|e| {
            log::warn!("cannot parse memory total '{s}': {e}; using fallback 2048");
            2048
        })
    } else {
        log::warn!("memory stats output missing total field; using fallback 2048");
        2048
    };

    Ok((cpu_percent, used, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_name() {
        assert_eq!(container_name("my-project").unwrap(), "opnble-my-project");
    }

    #[test]
    fn test_container_name_rejects_empty() {
        assert!(container_name("").is_err());
    }

    #[test]
    fn test_container_name_rejects_invalid_chars() {
        assert!(container_name("foo/bar").is_err());
        assert!(container_name("foo bar").is_err());
        assert!(container_name("../etc").is_err());
    }

    #[test]
    fn test_validate_project_id_exposed() {
        assert!(validate_project_id("ok").is_ok());
        assert!(validate_project_id("project-123").is_ok());
        assert!(validate_project_id("").is_err());
        assert!(validate_project_id("bad/chars").is_err());
    }

    #[test]
    fn test_parse_cpu_proc_stat() {
        let stat = "cpu  100 50 80 200 10 5 3 2 0 0\ncpu0 10 5 8 20 1 0 0 0 0 0\n";
        let tmp = std::env::temp_dir().join("opnble_test_proc_stat");
        std::fs::write(&tmp, stat).unwrap();
        let cpu = parse_cpu_proc_stat(tmp.to_str().unwrap()).unwrap();
        // busy = 100+50+80+10+5+3+2 = 250, total = 450, idle = 200
        // cpu% = 250/450 * 100 ≈ 55.56
        assert!((cpu - 55.5).abs() < 1.0);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_parse_mem_proc_meminfo() {
        let meminfo = "MemTotal:       16384000 kB
MemFree:         8000000 kB
MemAvailable:    10000000 kB
Buffers:           100000 kB
Cached:          2000000 kB
";
        let tmp = std::env::temp_dir().join("opnble_test_proc_meminfo");
        std::fs::write(&tmp, meminfo).unwrap();
        let (used, total) = parse_mem_proc_meminfo(tmp.to_str().unwrap()).unwrap();
        assert_eq!(total, 16_384_000 / 1024);
        assert_eq!(used, (16_384_000 - 10_000_000) / 1024);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_parse_mem_proc_meminfo_fallback_no_memavailable() {
        let meminfo = "MemTotal:       8192000 kB
MemFree:         4000000 kB
Buffers:           50000 kB
Cached:          1000000 kB
";
        let tmp = std::env::temp_dir().join("opnble_test_proc_meminfo2");
        std::fs::write(&tmp, meminfo).unwrap();
        let (used, total) = parse_mem_proc_meminfo(tmp.to_str().unwrap()).unwrap();
        assert_eq!(total, 8_192_000 / 1024);
        let available = 4_000_000 + 50_000 + 1_000_000;
        assert_eq!(used, (8_192_000 - available) / 1024);
        let _ = std::fs::remove_file(&tmp);
    }
}
