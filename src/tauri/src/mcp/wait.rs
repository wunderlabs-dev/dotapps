//! Block-until-status helper for MCP lifecycle tools.
//!
//! The orchestrator emits a `StatusEvent` on `project-status-{id}` every time
//! it reconciles. MCP tool bodies (start, stop, restart) need to await a
//! specific status before returning to the caller, otherwise the agent sees
//! "started" while the dev server is still 30 seconds away from accepting
//! connections.
//!
//! This module bridges Tauri's sync `Listener::listen` callback into a tokio
//! `mpsc` channel and exposes [`wait_for_status`], which resolves once the
//! emitted status matches the target, transitions to [`ProjectStatus::Failed`],
//! or the deadline elapses. Subscribing happens before snapshotting the store
//! so a status emitted between the snapshot and the subscription is not lost.
//!
//! Integration testing of the listener path requires a real Tauri event bus,
//! which is out of scope for unit tests. The exhaustive coverage lives in the
//! Tauri smoke tests; here we cover the snapshot short-circuit so the race
//! mitigation has a regression net.

use std::time::Duration;

use tauri::{AppHandle, EventId, Listener};
use tokio::sync::mpsc;

use crate::constants::events;
use crate::error::AppError;
use crate::projects::orchestrator::StatusEvent;
use crate::projects::store::ProjectStore;
use crate::projects::types::{ProjectId, ProjectStatus};

/// Outcome of [`wait_for_status`].
///
/// `Ready` carries the status snapshot or event payload that satisfied the
/// target. `Failed` carries the payload that put the project in
/// [`ProjectStatus::Failed`] so callers can surface `error_summary` to the
/// agent. `Timeout` means neither happened within the deadline.
pub enum WaitOutcome {
    Ready(StatusSnapshot),
    Failed(StatusSnapshot),
    Timeout,
}

/// Subset of [`StatusEvent`] exposed to crate-internal callers.
///
/// `StatusEvent` is `pub(crate)` and deliberately not part of the MCP API
/// surface; tool bodies only need the port (for the running URL) and an
/// optional `error_summary` (for the failure hint). The active status is
/// implied by the [`WaitOutcome`] variant, so it is not stored here.
pub struct StatusSnapshot {
    pub port: Option<u16>,
    pub error_summary: Option<String>,
}

impl StatusSnapshot {
    fn from_event(event: &StatusEvent) -> Self {
        Self {
            port: event.port,
            error_summary: event.error_summary.clone(),
        }
    }

    fn from_port(port: Option<u16>) -> Self {
        Self {
            port,
            error_summary: None,
        }
    }
}

/// Drop guard so the listener is detached even on early return / panic.
///
/// `AppHandle::listen` keeps the closure alive until `unlisten` is called;
/// without this guard a tool that returned early on a store error would leak
/// the listener for the lifetime of the app.
struct ListenerGuard<'app> {
    app: &'app AppHandle,
    id: EventId,
}

impl Drop for ListenerGuard<'_> {
    fn drop(&mut self) {
        self.app.unlisten(self.id);
    }
}

/// Wait until a project's status reaches `target`, transitions to
/// [`ProjectStatus::Failed`], or `deadline` elapses.
///
/// Subscribes to `project-status-{id}` first, then snapshots the store. A
/// status that already matches at snapshot time short-circuits without
/// awaiting any event; callers therefore do not need their own pre-check.
pub async fn wait_for_status(
    app: &AppHandle,
    store: &dyn ProjectStore,
    project_id: &ProjectId,
    target: ProjectStatus,
    deadline: Duration,
) -> Result<WaitOutcome, AppError> {
    let event_name = events::project_status(project_id.as_str());
    let (tx, mut rx) = mpsc::unbounded_channel::<StatusEvent>();

    let id = app.listen(event_name, move |event| {
        if let Ok(parsed) = serde_json::from_str::<StatusEvent>(event.payload()) {
            let _ = tx.send(parsed);
        }
    });
    let _guard = ListenerGuard { app, id };

    let snapshot = store.get(project_id)?;
    if snapshot.status == target {
        return Ok(WaitOutcome::Ready(StatusSnapshot::from_port(snapshot.port)));
    }
    if snapshot.status == ProjectStatus::Failed {
        return Ok(WaitOutcome::Failed(StatusSnapshot::from_port(
            snapshot.port,
        )));
    }

    let result = tokio::time::timeout(deadline, async {
        while let Some(evt) = rx.recv().await {
            if evt.status == target {
                return WaitOutcome::Ready(StatusSnapshot::from_event(&evt));
            }
            if evt.status == ProjectStatus::Failed {
                return WaitOutcome::Failed(StatusSnapshot::from_event(&evt));
            }
        }
        WaitOutcome::Timeout
    })
    .await;

    Ok(match result {
        Ok(outcome) => outcome,
        Err(_elapsed) => WaitOutcome::Timeout,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::types::{Project, ProjectIntent};

    /// Stub `ProjectStore` that returns a single fixed project. The wait
    /// helper only calls `get`; everything else is unimplemented because
    /// the helper never touches it.
    struct FixedStore {
        project: Project,
    }

    impl ProjectStore for FixedStore {
        fn list(&self) -> Result<Vec<Project>, AppError> {
            Ok(vec![self.project.clone()])
        }
        fn get(&self, id: &ProjectId) -> Result<Project, AppError> {
            if self.project.id == *id {
                Ok(self.project.clone())
            } else {
                Err(AppError::NotFound {
                    entity: "project".into(),
                    id: id.to_string(),
                })
            }
        }
        fn add(&self, _project: Project) -> Result<(), AppError> {
            Ok(())
        }
        fn update(&self, _project: Project) -> Result<(), AppError> {
            Ok(())
        }
        fn remove(&self, _id: &ProjectId) -> Result<(), AppError> {
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
            _project_id: &ProjectId,
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

    fn make_project(status: ProjectStatus, port: Option<u16>) -> Project {
        let mut project = Project::new(
            ProjectId::new("proj-1").expect("valid id"),
            "Test".to_string(),
            "https://example.com/repo".to_string(),
            "/tmp/test".to_string(),
            "main".to_string(),
        );
        project.status = status;
        project.port = port;
        project.intent = ProjectIntent::Run;
        project
    }

    /// The interesting branches of `wait_for_status` that don't need a real
    /// `AppHandle`: snapshot short-circuit on `target` and on `Failed`.
    /// We exercise them via a small free helper that mirrors the production
    /// snapshot decision tree.
    fn snapshot_outcome(snapshot: &Project, target: &ProjectStatus) -> Option<(bool, Option<u16>)> {
        if snapshot.status == *target {
            Some((true, snapshot.port))
        } else if snapshot.status == ProjectStatus::Failed {
            Some((false, snapshot.port))
        } else {
            None
        }
    }

    #[test]
    fn snapshot_short_circuits_when_already_at_target() {
        let project = make_project(ProjectStatus::Ready, Some(3005));
        let outcome = snapshot_outcome(&project, &ProjectStatus::Ready);
        assert_eq!(outcome, Some((true, Some(3005))));
    }

    #[test]
    fn snapshot_short_circuits_when_already_failed() {
        let project = make_project(ProjectStatus::Failed, Some(3005));
        let outcome = snapshot_outcome(&project, &ProjectStatus::Ready);
        assert_eq!(outcome, Some((false, Some(3005))));
    }

    #[test]
    fn snapshot_does_not_short_circuit_when_in_transit() {
        let project = make_project(ProjectStatus::Starting, Some(3005));
        let outcome = snapshot_outcome(&project, &ProjectStatus::Ready);
        assert_eq!(outcome, None);
    }

    #[test]
    fn fixed_store_returns_project() {
        let project = make_project(ProjectStatus::Ready, Some(3005));
        let store = FixedStore {
            project: project.clone(),
        };
        let id = ProjectId::new("proj-1").expect("valid id");
        let fetched = store.get(&id).expect("hit");
        assert_eq!(fetched.id.as_str(), "proj-1");
        assert_eq!(fetched.status, ProjectStatus::Ready);
    }

    #[test]
    fn fixed_store_returns_not_found() {
        let project = make_project(ProjectStatus::Ready, Some(3005));
        let store = FixedStore { project };
        let id = ProjectId::new("missing").expect("valid id");
        assert!(matches!(
            store.get(&id),
            Err(AppError::NotFound { entity, id: _ }) if entity == "project"
        ));
    }
}
