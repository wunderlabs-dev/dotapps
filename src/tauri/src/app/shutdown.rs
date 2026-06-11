use std::sync::Arc;

use tauri::Manager;

use crate::infrastructure::Runtime;
use crate::projects::runner::RunningContainers;
use crate::projects::store::ProjectStore;
use crate::projects::{ProjectOrchestrator, ProjectSyncer};
use crate::tunnel::TunnelCoordinator;
#[cfg(target_os = "macos")]
use crate::vm::VmLifecycle;

/// Stop every running project while the runtime is still reachable.
pub async fn stop_all_projects(app: &tauri::AppHandle) {
    if let Some(syncer) = app.try_state::<Arc<ProjectSyncer>>() {
        syncer.stop_all_periodic().await;
    }
    if let Some(orchestrator) = app.try_state::<Arc<ProjectOrchestrator>>() {
        tracing::info!("Stopping all projects...");
        orchestrator.stop_all_projects(app, true).await;
    }
}

/// Gracefully shutdown containers and VM before exit.
/// Stops all orchestrated projects, cleans up tunnels, then the VM.
pub async fn graceful_shutdown(app: &tauri::AppHandle) {
    // Stop the listener first so no agent tool call lands mid-teardown.
    if let Some(mcp) = app.try_state::<crate::mcp::McpHandle>() {
        tracing::info!("Stopping MCP listener...");
        mcp.shutdown().await;
    }

    stop_all_projects(app).await;

    // Kill all cloudflared processes and delete tunnels via API
    if let (Some(tunnel), Some(store)) = (
        app.try_state::<Arc<TunnelCoordinator>>(),
        app.try_state::<Arc<dyn ProjectStore>>(),
    ) {
        tracing::info!("Stopping all active tunnels...");
        tunnel.stop_all(store.inner().as_ref()).await;
    }

    // Fallback: stop any containers the orchestrator doesn't know about
    if let (Some(running), Some(runtime)) = (
        app.try_state::<Arc<RunningContainers>>()
            .map(|state| Arc::clone(state.inner())),
        app.try_state::<Arc<dyn Runtime>>()
            .map(|state| Arc::clone(state.inner())),
    ) {
        let containers = running.list().await;
        for (project_id, container_id) in containers {
            tracing::info!("Stopping orphaned container for project {project_id}...");
            if let Err(e) = runtime.stop(&container_id).await {
                tracing::error!("cannot stop container {container_id}: {e}");
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(lifecycle) = app.try_state::<Arc<VmLifecycle>>() {
            tracing::info!("Stopping VM before exit...");
            if let Err(e) = lifecycle.stop().await {
                tracing::error!("cannot stop vm: {e}");
            } else {
                tracing::info!("VM stopped successfully");
            }
        }
    }
}
