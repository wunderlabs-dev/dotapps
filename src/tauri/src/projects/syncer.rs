//! Periodic and on-stop sync for projects
//!
//! Owns background sync workers and coordinates commit+push on project stop.

use std::collections::HashMap;
use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

use crate::auth::Authenticator;
use crate::error::AppError;
use crate::infrastructure::{
    default_commit_message, CommitAutoResult, GitOps, PushOutcome, RepoSyncStatus,
};
use crate::projects::orchestrator::ProjectOrchestrator;
use crate::projects::store::ProjectStore;
use crate::projects::types::{unix_now_millis, Project, ProjectId};
use crate::projects::{SyncEvent, SyncPolicy, SyncState};
use crate::settings::SettingsStore;

/// Shared dependencies for the periodic sync background worker.
///
/// Groups the `Arc<dyn T>` service dependencies that the sync loop needs,
/// avoiding 4+ separate `Arc` parameters on the free function.
struct SyncContext {
    app: AppHandle,
    store: Arc<dyn ProjectStore>,
    settings_store: Arc<dyn SettingsStore>,
    git: Arc<dyn GitOps>,
    authenticator: Arc<Authenticator>,
}

/// Background sync worker pool and on-stop sync logic for projects.
pub struct ProjectSyncer {
    store: Arc<dyn ProjectStore>,
    settings_store: Arc<dyn SettingsStore>,
    git: Arc<dyn GitOps>,
    authenticator: Arc<Authenticator>,
    workers: tokio::sync::Mutex<HashMap<String, JoinHandle<()>>>,
}

impl ProjectSyncer {
    pub fn new(
        store: Arc<dyn ProjectStore>,
        settings_store: Arc<dyn SettingsStore>,
        git: Arc<dyn GitOps>,
        authenticator: Arc<Authenticator>,
    ) -> Self {
        Self {
            store,
            settings_store,
            git,
            authenticator,
            workers: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Stop the background worker, stop the project, and optionally push.
    pub async fn stop_with_sync(
        &self,
        app: &AppHandle,
        orchestrator: &ProjectOrchestrator,
        project_id: &ProjectId,
    ) -> Result<(), AppError> {
        self.stop_periodic(project_id.as_str()).await;
        orchestrator.stop_project(project_id.as_str()).await?;

        let policy = match self.settings_store.get() {
            Ok(settings) => SyncPolicy::from(&settings),
            Err(e) => {
                tracing::warn!(
                    project_id = %project_id,
                    error = %e,
                    "skipping stop-time sync: cannot read settings"
                );
                return Ok(());
            }
        };

        if !policy.enabled || !policy.push_on_stop {
            Self::emit(
                app,
                project_id.as_str(),
                SyncState::Idle,
                Some("Auto-sync on stop is disabled."),
            );
            return Ok(());
        }

        let project = match self.store.get(project_id) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    project_id = %project_id,
                    error = %e,
                    "skipping stop-time sync: cannot load project"
                );
                Self::emit(
                    app,
                    project_id.as_str(),
                    SyncState::Error,
                    Some("cannot load project for sync."),
                );
                return Ok(());
            }
        };

        if let Err(e) = self.sync_on_stop(app, &project).await {
            tracing::error!(
                project_id = %project_id,
                error = %e,
                "project stopped, but stop-time sync failed"
            );
        }

        Ok(())
    }

    /// Spawn a periodic background sync worker for the project.
    pub async fn start_periodic(&self, app: &AppHandle, project_id: &str) {
        self.stop_periodic(project_id).await;

        let pid = project_id.to_string();
        let ctx = SyncContext {
            app: app.clone(),
            store: Arc::clone(&self.store),
            settings_store: Arc::clone(&self.settings_store),
            git: Arc::clone(&self.git),
            authenticator: Arc::clone(&self.authenticator),
        };

        let worker_pid = pid.clone();
        let handle = tokio::spawn(async move {
            periodic_loop(ctx, worker_pid).await;
        });

        self.workers.lock().await.insert(pid, handle);
    }

    /// Delegate to the underlying store for port allocation.
    pub fn allocate_port(&self) -> Result<u16, AppError> {
        self.store.allocate_port()
    }

    /// Cancel and remove the periodic sync worker for a project.
    pub async fn stop_periodic(&self, project_id: &str) {
        if let Some(handle) = self.workers.lock().await.remove(project_id) {
            handle.abort();
        }
    }

    /// Cancel every periodic sync worker.
    pub async fn stop_all_periodic(&self) {
        let mut workers = self.workers.lock().await;
        for (_id, handle) in workers.drain() {
            handle.abort();
        }
    }

    /// Commit local changes and push to remote.
    async fn sync_on_stop(&self, app: &AppHandle, project: &Project) -> Result<(), AppError> {
        let pid = project.id.as_str();
        let repo_path = project.repo_path();

        Self::emit(app, pid, SyncState::Committing, None);
        let commit_result = self.git.commit_auto(&repo_path, default_commit_message())?;
        if matches!(commit_result, CommitAutoResult::Committed) {
            Self::emit(
                app,
                pid,
                SyncState::Committing,
                Some("Saved local changes."),
            );
        }

        Self::emit(app, pid, SyncState::Pushing, None);
        let token = match self
            .authenticator
            .resolve_push_token(&project.repo_url)
            .await
        {
            Ok(t) => t,
            Err(ref e) if matches!(e, AppError::AuthRequired { .. }) => {
                tracing::warn!(
                    project_id = %pid,
                    "sync-on-stop: auth required, skipping push"
                );
                Self::emit(
                    app,
                    pid,
                    SyncState::AuthRequired,
                    Some("Sign in required to sync."),
                );
                return Ok(());
            }
            Err(e) => {
                tracing::warn!(
                    project_id = %pid,
                    error = %e,
                    "sync-on-stop: token resolution failed"
                );
                Self::emit(app, pid, SyncState::Error, Some(&e.to_string()));
                return Ok(());
            }
        };

        let push_result = self.git.push_current_branch(&repo_path, token.as_deref())?;
        match push_result {
            PushOutcome::Pushed | PushOutcome::NothingToPush => {}
            PushOutcome::Failed(reason) => {
                tracing::warn!(
                    project_id = %pid,
                    reason = %reason,
                    "sync-on-stop: push failed"
                );
                let state = classify_push_failure(&reason);
                Self::emit(app, pid, state, Some(&reason));
                return Ok(());
            }
        }

        let mut updated = project.clone();
        updated.last_synced_at = unix_now_millis();
        if let Err(e) = self.store.update(updated) {
            tracing::warn!(
                project_id = %pid,
                error = %e,
                "sync-on-stop: cannot persist last_synced_at"
            );
        }

        let status = self.git.repo_sync_status(&repo_path)?;
        match status {
            RepoSyncStatus::Clean => Self::emit(app, pid, SyncState::Synced, None),
            RepoSyncStatus::Ahead => Self::emit(
                app,
                pid,
                SyncState::PendingRemote,
                Some("Changes are saved locally and pending upload."),
            ),
            RepoSyncStatus::Behind | RepoSyncStatus::Diverged => Self::emit(
                app,
                pid,
                SyncState::Conflict,
                Some("Remote has conflicting changes. Pull and resolve conflicts."),
            ),
        }

        Ok(())
    }

    fn emit(app: &AppHandle, project_id: &str, state: SyncState, detail: Option<&str>) {
        let mut event = SyncEvent::new(project_id, state);
        if let Some(d) = detail {
            event = event.with_detail(d);
        }
        let event_name = crate::constants::events::project_sync(project_id);
        let _ = app.emit(&event_name, event);
    }
}

fn classify_push_failure(reason: &str) -> SyncState {
    let lower = reason.to_ascii_lowercase();
    if lower.contains("auth")
        || lower.contains("401")
        || lower.contains("403")
        || lower.contains("credential")
        || lower.contains("permission denied")
    {
        SyncState::AuthRequired
    } else if lower.contains("non-fast-forward")
        || lower.contains("fetch first")
        || lower.contains("rejected")
    {
        SyncState::Conflict
    } else {
        SyncState::Error
    }
}

/// Inner loop for the periodic background worker (runs inside a spawned task).
async fn periodic_loop(ctx: SyncContext, project_id: String) {
    let syncer = ProjectSyncer::new(
        Arc::clone(&ctx.store),
        Arc::clone(&ctx.settings_store),
        Arc::clone(&ctx.git),
        Arc::clone(&ctx.authenticator),
    );
    let mut first_run = true;

    loop {
        let policy = match ctx.settings_store.get() {
            Ok(settings) => SyncPolicy::from(&settings),
            Err(e) => {
                tracing::warn!(
                    project_id = %project_id,
                    error = %e,
                    "periodic worker: cannot load settings; will retry"
                );
                sleep(Duration::from_secs(30)).await;
                continue;
            }
        };

        if !policy.enabled {
            sleep(Duration::from_secs(30)).await;
            continue;
        }

        let wait = if first_run {
            policy.debounce()
        } else {
            policy.push_interval()
        };
        first_run = false;
        sleep(wait).await;

        let project_key = match ProjectId::new(project_id.clone()) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!(
                    project_id = %project_id,
                    error = %e,
                    "periodic worker: invalid project id; exiting"
                );
                break;
            }
        };

        let Ok(project) = ctx.store.get(&project_key) else {
            break;
        };

        if !project.status.is_syncable() {
            break;
        }

        if let Err(e) = syncer.sync_on_stop(&ctx.app, &project).await {
            tracing::warn!(
                project_id = %project_id,
                error = %e,
                "periodic sync failed; will retry on next tick"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::classify_push_failure;
    use crate::projects::SyncState;

    #[test]
    fn classify_auth_failure() {
        assert_eq!(
            classify_push_failure("authentication failed"),
            SyncState::AuthRequired
        );
    }

    #[test]
    fn classify_conflict() {
        assert_eq!(
            classify_push_failure("[rejected] non-fast-forward"),
            SyncState::Conflict
        );
    }

    #[test]
    fn classify_generic_error() {
        assert_eq!(
            classify_push_failure("something went wrong"),
            SyncState::Error
        );
    }
}
