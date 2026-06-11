//! Trait that each target OS implements to wire up its container runtime
//! and Tauri state.
//!
//! `bootstrap::run()` selects the implementation with a single `#[cfg]` block
//! and then calls these methods in order.

use std::sync::Arc;

use tauri::{AppHandle, Builder, CloseRequestApi, Window, Wry};

use crate::error::AppError;
use crate::infrastructure::Runtime;
use crate::mcp::exec::{ExecRunner, UnavailableExecRunner};

/// Implement for each target OS to wire up the container runtime and Tauri
/// state.
///
/// Each target OS provides an implementation that creates its runtime,
/// registers platform-specific Tauri state, and runs setup logic.
pub trait PlatformBootstrap: Send + Sync {
    /// Call to obtain the container runtime for the current platform.
    ///
    /// macOS: `VmRuntime` (containers in VM via Podman).
    /// Linux: `PodmanRuntime` (native containers).
    /// Windows: `WSL2Runtime` (containers via WSL Podman).
    fn runtime(&self) -> Arc<dyn Runtime>;

    /// Call to register platform-specific state with the Tauri builder.
    ///
    /// macOS: `VmLifecycle`. Windows: WSL2 mutex. Linux: nothing.
    fn manage_state(&self, builder: Builder<Wry>) -> Builder<Wry>;

    /// Call to obtain the `ExecRunner` for the `exec_command` MCP tool.
    ///
    /// macOS: an `AgentExecRunner` sharing `VmLifecycle`'s manager so it
    /// resolves the live `AgentClient` lazily (the agent does not exist
    /// until the VM starts).
    /// Linux/Windows: a stub that reports `VmNotRunning`, because
    /// `exec_command` is contracted on the VM agent.
    fn exec_runner(&self) -> Arc<dyn ExecRunner> {
        Arc::new(UnavailableExecRunner)
    }

    /// Override to run platform-specific logic after Tauri manages all shared
    /// state.
    ///
    /// macOS: auto-start VM, reconcile projects, clean orphaned tunnels.
    /// Default: no-op.
    #[expect(unused_variables, reason = "default no-op does not use app")]
    fn on_setup(&self, app: &AppHandle) -> Result<(), AppError> {
        Ok(())
    }

    /// Override to intercept the window close event on platforms that hide to
    /// the dock.
    ///
    /// Default: do nothing (window closes normally).
    #[expect(unused_variables, reason = "default no-op does not use params")]
    fn on_close_requested(&self, window: &Window, api: &CloseRequestApi) {}
}
