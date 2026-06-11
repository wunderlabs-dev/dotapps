//! `ExecRunner` trait abstraction over `agent_client::exec_host`.
//!
//! Lets the `exec_command` MCP tool body be tested without a live VM. The
//! production impl on macOS lazily resolves the agent through the live
//! `VmGateway` (which is constructed only after the VM starts); a stub impl
//! is used on platforms without an agent.
//!
//! Tests substitute their own `ExecRunner` implementation directly when
//! constructing an `OpnbleMcp` for a tool-body test.

use std::sync::Arc;

use async_trait::async_trait;

use crate::error::AppError;

/// Runs an already-wrapped shell command (timeout + `podman exec` wrapper as
/// produced by `build_exec_wrapper`) inside the VM and returns
/// `(exit_code, stdout, stderr)`.
#[async_trait]
pub trait ExecRunner: Send + Sync {
    async fn run(&self, wrapped_cmd: &str) -> Result<(i32, String, String), AppError>;
}

/// Production `ExecRunner` on macOS: holds the same `Arc<Mutex<Option<VmGateway>>>`
/// the `VmRuntime` does, so it resolves the live `AgentClient` at each call
/// instead of capturing one at app-boot time (the agent does not exist until
/// the VM starts).
#[cfg(target_os = "macos")]
pub struct AgentExecRunner {
    manager: Arc<tokio::sync::Mutex<Option<crate::vm::VmGateway>>>,
}

#[cfg(target_os = "macos")]
impl AgentExecRunner {
    pub fn new(manager: Arc<tokio::sync::Mutex<Option<crate::vm::VmGateway>>>) -> Self {
        Self { manager }
    }
}

#[cfg(target_os = "macos")]
#[async_trait]
impl ExecRunner for AgentExecRunner {
    async fn run(&self, wrapped_cmd: &str) -> Result<(i32, String, String), AppError> {
        // Clone the agent Arc while briefly holding the lock, then release
        // so the RPC does not pin the manager mutex for its full duration.
        let agent = {
            let guard = self.manager.lock().await;
            let manager = guard.as_ref().ok_or(AppError::VmNotRunning)?;
            Arc::clone(manager.agent())
        };
        agent.exec_host(wrapped_cmd).await
    }
}

/// Stub `ExecRunner` for platforms without an agent (Linux native, Windows
/// WSL2). `exec_command` is wired to the VM agent contract, so on these
/// platforms the tool reports `VmNotRunning` rather than silently routing
/// through a different runtime surface.
pub struct UnavailableExecRunner;

#[async_trait]
impl ExecRunner for UnavailableExecRunner {
    async fn run(&self, _wrapped_cmd: &str) -> Result<(i32, String, String), AppError> {
        Err(AppError::VmNotRunning)
    }
}
