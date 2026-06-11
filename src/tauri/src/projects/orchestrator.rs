//! Project orchestrator: reconciliation loop per project
//!
//! Observes actual container state, compares to desired intent (run/stop),
//! and takes action to converge. Emits status events so the UI never lies.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::constants::events;
use crate::error::AppError;
use crate::infrastructure::runtime::Runtime;
use crate::projects::store::ProjectStore;
use crate::projects::types::{ProjectIntent, ProjectStatus};
use crate::tunnel::TunnelCoordinator;

/// Interval between health polls
const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Shared dependencies for the per-project reconciliation loop.
///
/// Groups the `Arc<dyn T>` service dependencies that every reconciler needs,
/// avoiding 4+ separate `Arc` parameters on free functions.
struct ReconcileContext {
    app: AppHandle,
    runtime: Arc<dyn Runtime>,
    store: Arc<dyn ProjectStore>,
    tunnel: Option<Arc<TunnelCoordinator>>,
}

/// Reconciler that drives projects toward their desired intent.
///
/// Spawns one tokio task per project with intent=Run. Each task polls
/// health, derives status, emits events, and triggers recovery actions.
pub struct ProjectOrchestrator {
    runtime: Arc<dyn Runtime>,
    store: Arc<dyn ProjectStore>,
    tunnel: Option<Arc<TunnelCoordinator>>,
    tasks: Mutex<HashMap<String, JoinHandle<()>>>,
}

impl ProjectOrchestrator {
    pub fn new(
        runtime: Arc<dyn Runtime>,
        store: Arc<dyn ProjectStore>,
        tunnel: Option<Arc<TunnelCoordinator>>,
    ) -> Self {
        Self {
            runtime,
            store,
            tunnel,
            tasks: Mutex::new(HashMap::new()),
        }
    }

    /// Access the runtime for direct operations (e.g., install flow).
    pub fn runtime(&self) -> Arc<dyn Runtime> {
        Arc::clone(&self.runtime)
    }

    /// Set intent to Run and spawn a reconciliation loop for the project.
    pub async fn start_project(
        &self,
        app: &AppHandle,
        project_id: &str,
        port: u16,
    ) -> Result<(), AppError> {
        // Persist intent and port so both survive app restart
        let id = crate::projects::types::ProjectId::new(project_id)?;
        let mut project = self.store.get(&id)?;
        project.intent = ProjectIntent::Run;
        project.port = Some(port);
        project.status = ProjectStatus::Starting;
        self.store.update(project)?;

        self.spawn_reconciler(app.clone(), project_id.to_string(), port)
            .await;
        Ok(())
    }

    /// Set intent to Stop. The running reconciler detects this and tears down.
    ///
    /// Async because callers need to await it in an async context, even though
    /// the current implementation is synchronous. The reconciler handles actual
    /// teardown asynchronously on its next tick.
    #[expect(
        clippy::unused_async,
        reason = "callers await this; teardown happens in the reconciler's async loop"
    )]
    pub async fn stop_project(&self, project_id: &str) -> Result<(), AppError> {
        self.set_intent(project_id, ProjectIntent::Stop)?;

        // The reconciler will detect intent=Stop on its next tick
        // and handle teardown, then exit. We don't abort it here
        // because it needs to clean up gracefully.
        Ok(())
    }

    /// Force-stop and remove a project's container synchronously.
    ///
    /// Unlike `stop_project` which delegates to the reconciler, this method
    /// stops and removes the container immediately. Used by restart and delete
    /// flows that need the container gone before proceeding.
    pub async fn force_stop_and_remove(&self, project_id: &str) -> Result<(), AppError> {
        // Abort the reconciler so it doesn't interfere
        {
            let mut tasks = self.tasks.lock().await;
            if let Some(handle) = tasks.remove(project_id) {
                handle.abort();
            }
        }

        // Stop and remove directly. Errors are logged but not propagated
        // because the container may already be stopped or absent.
        if let Err(e) = self.runtime.stop(project_id).await {
            tracing::debug!(
                project_id = %project_id,
                error = %e,
                "cannot stop container (may already be stopped)"
            );
        }
        if let Err(e) = self.runtime.remove(project_id).await {
            tracing::debug!(
                project_id = %project_id,
                error = %e,
                "cannot remove container (may not exist)"
            );
        }

        // Persist the stopped state
        let id = crate::projects::types::ProjectId::new(project_id)?;
        if let Ok(mut project) = self.store.get(&id) {
            project.intent = ProjectIntent::Stop;
            project.status = ProjectStatus::Stopped;
            project.port = None;
            if let Err(e) = self.store.update(project) {
                tracing::warn!(
                    project_id = %project_id,
                    error = %e,
                    "cannot persist stopped state"
                );
            }
        }

        Ok(())
    }

    /// Stop all running reconcilers (for graceful app shutdown).
    pub async fn stop_all(&self) {
        let mut tasks = self.tasks.lock().await;
        for (_id, handle) in tasks.drain() {
            handle.abort();
        }
    }

    /// Persist stopped intent and status for every project.
    ///
    /// Synchronous so the first frontend `state()` call after launch never
    /// reads stale runtime fields from the previous session.
    pub fn mark_all_stopped_in_store(&self) -> Result<(), AppError> {
        for mut project in self.store.list()? {
            apply_stopped_fields(&mut project);
            self.store.update(project)?;
        }
        Ok(())
    }

    /// Stop every project and persist `Stopped` state.
    ///
    /// When `stop_containers` is true, attempts runtime teardown first. Call
    /// that while the VM is still up. On app launch pass false: containers
    /// from the previous session are already gone.
    pub async fn stop_all_projects(&self, app: &AppHandle, stop_containers: bool) {
        self.stop_all().await;

        let projects = match self.store.list() {
            Ok(projects) => projects,
            Err(e) => {
                tracing::error!(error = %e, "cannot load projects to stop");
                return;
            }
        };

        for project in &projects {
            let project_id = project.id.as_str();

            if stop_containers {
                if let Err(e) = self.runtime.stop(project_id).await {
                    tracing::debug!(
                        project_id = %project_id,
                        error = %e,
                        "cannot stop container (may already be stopped)"
                    );
                }
            }

            if let Some(ref tunnel) = self.tunnel {
                if project.tunnel_id.is_some() || project.tunnel_url.is_some() {
                    if let Err(e) = tunnel.unshare(&project.id, "", self.store.as_ref()).await {
                        tracing::warn!(
                            project_id = %project_id,
                            error = %e,
                            "cannot unshare tunnel"
                        );
                    }
                }
            }
        }

        if let Err(e) = self.mark_all_stopped_in_store() {
            tracing::error!(error = %e, "cannot persist stopped state for all projects");
            return;
        }

        let projects = match self.store.list() {
            Ok(projects) => projects,
            Err(e) => {
                tracing::error!(error = %e, "cannot load projects to emit stopped status");
                return;
            }
        };

        for project in projects {
            let payload = StatusEvent::from_project(&project, project.pages_url.clone());
            emit_status_full(app, project.id.as_str(), &payload);
        }
    }

    /// Persist the intent change to the store.
    fn set_intent(&self, project_id: &str, intent: ProjectIntent) -> Result<(), AppError> {
        let id = crate::projects::types::ProjectId::new(project_id)?;
        let mut project = self.store.get(&id)?;
        project.intent = intent;
        self.store.update(project)
    }

    /// Spawn (or replace) the reconciler task for a project.
    async fn spawn_reconciler(&self, app: AppHandle, project_id: String, port: u16) {
        let mut tasks = self.tasks.lock().await;

        // Abort any existing reconciler for this project
        if let Some(existing) = tasks.remove(&project_id) {
            existing.abort();
        }

        let ctx = ReconcileContext {
            app,
            runtime: Arc::clone(&self.runtime),
            store: Arc::clone(&self.store),
            tunnel: self.tunnel.as_ref().map(Arc::clone),
        };
        let id_for_task = project_id.clone();

        let handle = tokio::spawn(async move {
            reconcile_loop(ctx, id_for_task, port).await;
        });

        tasks.insert(project_id, handle);
    }
}

/// The per-project reconciliation loop.
///
/// Polls container health and derives status. Emits Tauri events when
/// status changes. Exits when intent becomes Stop and teardown completes.
async fn reconcile_loop(ctx: ReconcileContext, project_id: String, port: u16) {
    let mut previous_status = ProjectStatus::Starting;
    let mut previous_tunnel_live = false;

    // Parse project ID once, not every iteration
    let project_id_typed = match crate::projects::types::ProjectId::new(&project_id) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!(
                project_id = %project_id,
                error = %e,
                "cannot parse project id in reconciler"
            );
            return;
        }
    };

    // Emit initial Starting status
    emit_status(&ctx, &project_id_typed, &previous_status, port);

    loop {
        tokio::time::sleep(POLL_INTERVAL).await;

        // 1. Read current intent from store
        let project = match ctx.store.get(&project_id_typed) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(
                    project_id = %project_id,
                    error = %e,
                    "cannot load project in reconciler"
                );
                // Project may have been deleted; exit the reconciler
                return;
            }
        };

        // 2. If intent is Stop, handle teardown
        if project.intent == ProjectIntent::Stop {
            let new_status = handle_stop_intent(
                &ctx.runtime,
                &ctx.store,
                &project_id,
                &project_id_typed,
                &previous_status,
            )
            .await;

            if new_status != previous_status {
                emit_status(&ctx, &project_id_typed, &new_status, port);
                persist_status(&ctx.store, &project_id_typed, &new_status);
                previous_status = new_status.clone();
            }

            if new_status == ProjectStatus::Stopped {
                // Auto-unshare if tunnel is active
                if let Some(ref tunnel) = ctx.tunnel {
                    if let Err(e) = tunnel
                        .unshare(&project_id_typed, "", ctx.store.as_ref())
                        .await
                    {
                        tracing::warn!(
                            project_id = %project_id,
                            error = %e,
                            "cannot auto-unshare project"
                        );
                    }
                }
                tracing::info!(project_id = %project_id, "reconciler exiting after stop");
                return;
            }

            // Still tearing down, loop again
            continue;
        }

        // 3. Check runtime reachability before health polling.
        //    On macOS/Windows, the VM must be running for the agent to respond.
        //    If unreachable, emit WaitingForVm and skip this tick.
        if !ctx.runtime.is_reachable().await {
            let new_status = ProjectStatus::WaitingForVm;
            if new_status != previous_status {
                emit_status(&ctx, &project_id_typed, &new_status, port);
                persist_status(&ctx.store, &project_id_typed, &new_status);
                previous_status = new_status;
            }
            continue;
        }

        // 4. Intent is Run: ensure container is started, then poll health
        let container_running = ctx.runtime.is_running(&project_id).await;

        // If container is not running and we should be starting or retrying, start it.
        // WaitingForVm is included because we just passed the reachability check above,
        // meaning the runtime became available and the container needs starting.
        if !container_running
            && matches!(
                previous_status,
                ProjectStatus::Starting
                    | ProjectStatus::Failed
                    | ProjectStatus::FixFailed
                    | ProjectStatus::WaitingForVm
            )
        {
            previous_status = try_start_container(
                &ctx,
                &project_id,
                &project_id_typed,
                &project.repo_path(),
                port,
            )
            .await;
            continue;
        }

        let http_reachable = probe_http_health(port).await;

        // 5. Derive new status from observed signals
        let new_status = derive_status(&previous_status, container_running, http_reachable);

        // 6. Look up current tunnel state (if shared)
        let (current_tunnel_url, tunnel_live) = if let Some(ref tunnel) = ctx.tunnel {
            (
                tunnel.tunnel_url(&project_id_typed).await,
                tunnel.is_tunnel_live(&project_id_typed).await,
            )
        } else {
            (None, false)
        };

        // 7. Emit event if status changed OR tunnel became live
        let tunnel_state_changed = tunnel_live && !previous_tunnel_live;
        if new_status != previous_status || tunnel_state_changed {
            emit_status_full(
                &ctx.app,
                &project_id,
                &StatusEvent {
                    status: new_status.clone(),
                    port: Some(port),
                    error_summary: None,
                    fix_attempt: None,
                    tunnel_url: current_tunnel_url,
                    tunnel_live: Some(tunnel_live),
                    pages_url: project.pages_url.clone(),
                },
            );
            persist_status(&ctx.store, &project_id_typed, &new_status);
            previous_status = new_status;
            previous_tunnel_live = tunnel_live;
        }
    }
}

/// Attempt to start the container, emitting and persisting the resulting status.
async fn try_start_container(
    ctx: &ReconcileContext,
    project_id: &str,
    project_id_typed: &crate::projects::types::ProjectId,
    repo_path: &std::path::Path,
    port: u16,
) -> ProjectStatus {
    tracing::info!(project_id = %project_id, port, "starting container");
    let new_status = match ctx.runtime.start(project_id, repo_path, port).await {
        Ok(()) => {
            tracing::info!(project_id = %project_id, "container started");
            ProjectStatus::Running
        }
        Err(e) => {
            tracing::error!(
                project_id = %project_id,
                error = %e,
                "cannot start container"
            );
            ProjectStatus::Failed
        }
    };
    emit_status(ctx, project_id_typed, &new_status, port);
    persist_status(&ctx.store, project_id_typed, &new_status);
    new_status
}

/// Derive the project status from observed health signals.
fn derive_status(
    previous: &ProjectStatus,
    container_running: bool,
    http_reachable: bool,
) -> ProjectStatus {
    if !container_running {
        // Container is not running
        return match previous {
            // If we were in a state where the container should be running,
            // it has failed
            ProjectStatus::Running
            | ProjectStatus::Ready
            | ProjectStatus::Degraded
            | ProjectStatus::Starting => ProjectStatus::Failed,
            // All other states: preserve current status.
            // Failed/FixFailed stay terminal, Fixing lets the pipeline drive,
            // Stopped/WaitingForVm/Stopping are not expected here but safe.
            ProjectStatus::Failed
            | ProjectStatus::FixFailed
            | ProjectStatus::Fixing
            | ProjectStatus::Stopped
            | ProjectStatus::WaitingForVm
            | ProjectStatus::Stopping => previous.clone(),
        };
    }

    // Container is running
    if http_reachable {
        ProjectStatus::Ready
    } else {
        // Container alive but HTTP not responding
        match previous {
            // Was previously serving HTTP, now degraded
            ProjectStatus::Ready => ProjectStatus::Degraded,
            // Otherwise still starting up or recovering
            ProjectStatus::Stopped
            | ProjectStatus::WaitingForVm
            | ProjectStatus::Starting
            | ProjectStatus::Running
            | ProjectStatus::Degraded
            | ProjectStatus::Failed
            | ProjectStatus::Fixing
            | ProjectStatus::FixFailed
            | ProjectStatus::Stopping => ProjectStatus::Running,
        }
    }
}

/// Handle the stop intent: tear down container and transition to Stopped.
async fn handle_stop_intent(
    runtime: &Arc<dyn Runtime>,
    store: &Arc<dyn ProjectStore>,
    project_id: &str,
    project_id_typed: &crate::projects::types::ProjectId,
    previous_status: &ProjectStatus,
) -> ProjectStatus {
    if *previous_status == ProjectStatus::Stopped {
        return ProjectStatus::Stopped;
    }

    // Transition to Stopping if not already
    if *previous_status != ProjectStatus::Stopping {
        persist_status(store, project_id_typed, &ProjectStatus::Stopping);
    }

    // Stop the container but keep it around for reuse on next start.
    // Containers are only removed on explicit project removal or restart.
    if let Err(e) = runtime.stop(project_id).await {
        tracing::error!(
            project_id = %project_id,
            error = %e,
            "cannot stop container"
        );
    }

    // Verify it's actually stopped
    if runtime.is_running(project_id).await {
        ProjectStatus::Stopping
    } else {
        ProjectStatus::Stopped
    }
}

/// Emit a project status event via Tauri.
/// Payload shape matching the frontend `ProjectStatusEvent` interface.
///
/// `Deserialize` exists so the MCP `wait_for_status` helper can listen on
/// `project-status-{id}` and parse the payload back into this type. Fields
/// stay `pub(crate)` because no consumer outside the crate reads them.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatusEvent {
    pub(crate) status: ProjectStatus,
    pub(crate) port: Option<u16>,
    pub(crate) error_summary: Option<String>,
    pub(crate) fix_attempt: Option<u32>,
    pub(crate) tunnel_url: Option<String>,
    pub(crate) tunnel_live: Option<bool>,
    pub(crate) pages_url: Option<String>,
}

impl StatusEvent {
    /// Build a status snapshot from the current project state,
    /// with an explicit pages URL (pass `project.pages_url.clone()`
    /// to preserve, or `None` to clear).
    pub(crate) fn from_project(
        project: &crate::projects::types::Project,
        pages_url: Option<String>,
    ) -> Self {
        Self {
            status: project.status.clone(),
            port: project.port,
            error_summary: None,
            fix_attempt: None,
            tunnel_url: project.tunnel_url.clone(),
            tunnel_live: None,
            pages_url,
        }
    }
}

fn emit_status(
    ctx: &ReconcileContext,
    project_id: &crate::projects::types::ProjectId,
    status: &ProjectStatus,
    port: u16,
) {
    let pages_url = ctx.store.get(project_id).ok().and_then(|p| p.pages_url);
    let payload = StatusEvent {
        status: status.clone(),
        port: Some(port),
        error_summary: None,
        fix_attempt: None,
        tunnel_url: None,
        tunnel_live: None,
        pages_url,
    };
    emit_event(&ctx.app, project_id.as_str(), &payload);
}

fn emit_status_full(app: &AppHandle, project_id: &str, payload: &StatusEvent) {
    emit_event(app, project_id, payload);
}

fn emit_event(app: &AppHandle, project_id: &str, payload: &StatusEvent) {
    let event_name = events::project_status(project_id);
    tracing::debug!(
        project_id = %project_id,
        status = %payload.status,
        "project status changed"
    );
    let _ = app.emit(&event_name, payload);
}

fn apply_stopped_fields(project: &mut crate::projects::types::Project) {
    project.intent = ProjectIntent::Stop;
    project.status = ProjectStatus::Stopped;
    project.tunnel_id = None;
    project.tunnel_url = None;
}

/// Persist the derived status to the store.
fn persist_status(
    store: &Arc<dyn ProjectStore>,
    project_id: &crate::projects::types::ProjectId,
    status: &ProjectStatus,
) {
    if let Ok(mut project) = store.get(project_id) {
        project.status = status.clone();
        if let Err(e) = store.update(project) {
            tracing::error!(
                project_id = %project_id,
                error = %e,
                "cannot persist status"
            );
        }
    }
}

/// Check if the dev server is responding to HTTP requests.
///
/// Any HTTP response (including 404 or 500) means the server is alive.
/// Only connection refused or timeout counts as "not ready," because
/// the port may accept TCP connections while npm install or compilation
/// is still running.
pub(crate) async fn probe_http_health(port: u16) -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .no_proxy()
        .build();

    let Ok(client) = client else {
        return false;
    };

    client
        .get(format!("http://127.0.0.1:{port}/"))
        .send()
        .await
        .is_ok()
}

#[cfg(test)]
#[expect(
    clippy::disallowed_types,
    reason = "sync-only in-memory store for unit tests, never held across await"
)]
mod tests {
    use std::path::Path;
    use std::sync::Mutex;

    use async_trait::async_trait;

    use super::*;
    use crate::infrastructure::Runtime;
    use crate::projects::types::Project;

    struct NoopRuntime;

    #[async_trait]
    impl Runtime for NoopRuntime {
        async fn start(
            &self,
            _project_id: &str,
            _repo_path: &Path,
            _host_port: u16,
        ) -> Result<(), AppError> {
            Ok(())
        }

        async fn stop(&self, _container_id: &str) -> Result<(), AppError> {
            Ok(())
        }

        async fn remove(&self, _container_id: &str) -> Result<(), AppError> {
            Ok(())
        }

        async fn is_running(&self, _container_id: &str) -> bool {
            false
        }

        async fn tail_logs(&self, _project_id: &str, _lines: u32) -> Result<Vec<String>, AppError> {
            Ok(Vec::new())
        }
    }

    struct RecordingStore {
        projects: Mutex<Vec<Project>>,
    }

    impl RecordingStore {
        fn new(projects: Vec<Project>) -> Self {
            Self {
                projects: Mutex::new(projects),
            }
        }
    }

    impl ProjectStore for RecordingStore {
        fn list(&self) -> Result<Vec<Project>, AppError> {
            Ok(self.projects.lock().expect("lock").clone())
        }

        fn get(&self, id: &crate::projects::types::ProjectId) -> Result<Project, AppError> {
            self.list()?
                .into_iter()
                .find(|project| project.id == *id)
                .ok_or_else(|| AppError::NotFound {
                    entity: "project".into(),
                    id: id.to_string(),
                })
        }

        fn add(&self, project: Project) -> Result<(), AppError> {
            self.projects.lock().expect("lock").push(project);
            Ok(())
        }

        fn update(&self, project: Project) -> Result<(), AppError> {
            let mut projects = self.projects.lock().expect("lock");
            let Some(index) = projects.iter().position(|entry| entry.id == project.id) else {
                return Err(AppError::NotFound {
                    entity: "project".into(),
                    id: project.id.to_string(),
                });
            };
            if let Some(slot) = projects.get_mut(index) {
                *slot = project;
            }
            Ok(())
        }

        fn remove(&self, id: &crate::projects::types::ProjectId) -> Result<(), AppError> {
            let mut projects = self.projects.lock().expect("lock");
            let len_before = projects.len();
            projects.retain(|project| project.id != *id);
            if projects.len() == len_before {
                return Err(AppError::NotFound {
                    entity: "project".into(),
                    id: id.to_string(),
                });
            }
            Ok(())
        }

        fn allocate_port(&self) -> Result<u16, AppError> {
            Ok(3001)
        }

        fn next_port(&self) -> Result<u16, AppError> {
            Ok(3001)
        }

        fn add_snapshot(&self, _record: crate::snapshots::SnapshotRecord) -> Result<(), AppError> {
            Ok(())
        }

        fn list_snapshots(
            &self,
            _project_id: &crate::projects::types::ProjectId,
        ) -> Result<Vec<crate::snapshots::SnapshotRecord>, AppError> {
            Ok(vec![])
        }

        fn remove_snapshot(&self, id: &crate::snapshots::SnapshotId) -> Result<(), AppError> {
            Err(AppError::NotFound {
                entity: "snapshot".into(),
                id: id.as_str().to_string(),
            })
        }

        fn get_snapshot(
            &self,
            id: &crate::snapshots::SnapshotId,
        ) -> Result<crate::snapshots::SnapshotRecord, AppError> {
            Err(AppError::NotFound {
                entity: "snapshot".into(),
                id: id.as_str().to_string(),
            })
        }
    }

    fn running_project(id: &str, port: u16) -> Project {
        let project_id = crate::projects::types::ProjectId::new(id).expect("valid id");
        let mut project = Project::new(
            project_id,
            "Test".to_string(),
            "https://example.com/repo".to_string(),
            format!("/tmp/{id}"),
            "main".to_string(),
        );
        project.intent = ProjectIntent::Run;
        project.status = ProjectStatus::Ready;
        project.port = Some(port);
        project.tunnel_id = Some("tunnel-1".to_string());
        project.tunnel_url = Some("https://preview.example.com".to_string());
        project
    }

    #[test]
    fn mark_all_stopped_resets_runtime_fields() {
        let store: Arc<dyn ProjectStore> = Arc::new(RecordingStore::new(vec![
            running_project("proj-a", 3005),
            running_project("proj-b", 3006),
        ]));
        let orchestrator =
            ProjectOrchestrator::new(Arc::new(NoopRuntime), Arc::clone(&store), None);

        orchestrator
            .mark_all_stopped_in_store()
            .expect("store update succeeds");

        let projects = store.list().expect("list succeeds");
        for project in projects {
            assert_eq!(project.intent, ProjectIntent::Stop);
            assert_eq!(project.status, ProjectStatus::Stopped);
            assert!(project.tunnel_id.is_none());
            assert!(project.tunnel_url.is_none());
        }
    }

    #[test]
    fn mark_all_stopped_preserves_port() {
        let store: Arc<dyn ProjectStore> =
            Arc::new(RecordingStore::new(vec![running_project("proj-a", 3005)]));
        let orchestrator =
            ProjectOrchestrator::new(Arc::new(NoopRuntime), Arc::clone(&store), None);

        orchestrator
            .mark_all_stopped_in_store()
            .expect("store update succeeds");

        let project = store
            .get(&crate::projects::types::ProjectId::new("proj-a").expect("valid id"))
            .expect("project exists");
        assert_eq!(project.port, Some(3005));
    }
}
