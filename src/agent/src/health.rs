use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::time::Duration;

use crate::container::{self, PodmanRuntime};
use crate::error::AgentError;

/// Raw health signals for a project, returned by the orchestrator health poll.
pub struct HealthResult {
    /// "none", "running", or "exited".
    pub container_state: String,
    /// Set when container has exited; 0 otherwise.
    pub exit_code: i32,
    /// Whether a TCP connection to 127.0.0.1:port succeeds within the timeout.
    pub http_reachable: bool,
}

const TCP_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Query container state via bollard inspect and TCP-probe the given port.
///
/// Returns combined health signals for the orchestrator reconciliation loop.
/// When the container does not exist, returns state "none" with no probe attempt.
pub async fn project_health(
    runtime: &PodmanRuntime,
    project_id: &str,
    port: u16,
) -> Result<HealthResult, AgentError> {
    container::validate_project_id(project_id).map_err(AgentError::other)?;
    let name = container::container_name(project_id).map_err(AgentError::other)?;

    let inspect_result = runtime.docker().inspect_container(&name, None).await;
    let response = match inspect_result {
        Ok(resp) => resp,
        Err(bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        }) => {
            return Ok(HealthResult {
                container_state: "none".to_string(),
                exit_code: 0,
                http_reachable: false,
            });
        }
        Err(source) => {
            return Err(AgentError::ContainerInspect { id: name, source });
        }
    };

    let (state_str, exit_code) = extract_state(&response);

    let http_reachable = if state_str == "running" && port > 0 {
        tcp_probe(port)
    } else {
        false
    };

    Ok(HealthResult {
        container_state: state_str,
        exit_code,
        http_reachable,
    })
}

/// Extract container state string and exit code from a bollard inspect response.
fn extract_state(response: &bollard::models::ContainerInspectResponse) -> (String, i32) {
    let Some(state) = &response.state else {
        return ("none".to_string(), 0);
    };

    let status = state.status.as_ref().map_or("none", |s| s.as_ref());

    // Normalize podman/docker status values to the three states the orchestrator expects.
    let normalized = match status {
        "exited" | "dead" | "stopped" => "exited",
        "running" | "created" | "paused" | "restarting" | "removing" => "running",
        _ => "none",
    };

    let exit_code = if normalized == "exited" {
        state
            .exit_code
            .and_then(|code| i32::try_from(code).ok())
            .unwrap_or(0)
    } else {
        0
    };

    (normalized.to_string(), exit_code)
}

/// Attempt a TCP connection to 127.0.0.1:port with a timeout.
///
/// Uses blocking I/O (this runs inside `block_on`, same as other agent RPCs).
fn tcp_probe(port: u16) -> bool {
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    TcpStream::connect_timeout(&addr.into(), TCP_PROBE_TIMEOUT).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_probe_unreachable_port_returns_false() {
        // Port 1 is almost certainly not listening; probe should return false quickly.
        assert!(!tcp_probe(1));
    }

    #[test]
    fn extract_state_none_when_no_state() {
        let response = bollard::models::ContainerInspectResponse {
            state: None,
            ..Default::default()
        };
        let (state, code) = extract_state(&response);
        assert_eq!(state, "none");
        assert_eq!(code, 0);
    }

    #[test]
    fn extract_state_running() {
        let response = bollard::models::ContainerInspectResponse {
            state: Some(bollard::models::ContainerState {
                status: Some(bollard::models::ContainerStateStatusEnum::RUNNING),
                exit_code: Some(0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (state, code) = extract_state(&response);
        assert_eq!(state, "running");
        assert_eq!(code, 0);
    }

    #[test]
    fn extract_state_exited_with_code() {
        let response = bollard::models::ContainerInspectResponse {
            state: Some(bollard::models::ContainerState {
                status: Some(bollard::models::ContainerStateStatusEnum::EXITED),
                exit_code: Some(137),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (state, code) = extract_state(&response);
        assert_eq!(state, "exited");
        assert_eq!(code, 137);
    }

    #[test]
    fn extract_state_dead_maps_to_exited() {
        let response = bollard::models::ContainerInspectResponse {
            state: Some(bollard::models::ContainerState {
                status: Some(bollard::models::ContainerStateStatusEnum::DEAD),
                exit_code: Some(1),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (state, code) = extract_state(&response);
        assert_eq!(state, "exited");
        assert_eq!(code, 1);
    }

    #[test]
    fn extract_state_created_maps_to_running() {
        let response = bollard::models::ContainerInspectResponse {
            state: Some(bollard::models::ContainerState {
                status: Some(bollard::models::ContainerStateStatusEnum::CREATED),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (state, code) = extract_state(&response);
        assert_eq!(state, "running");
        assert_eq!(code, 0);
    }
}
