//! Tauri IPC commands for the Cursor-integration settings panel.
//!
//! Two commands back the panel:
//!
//! - [`mcp_status`] reports the listener's bind URL, port, and how many
//!   imported projects have been linked into Cursor (i.e. own a slug, which
//!   is also the trigger for `.cursor/mcp.json` having been written).
//! - [`mcp_rotate_token`] regenerates the local bearer token on disk and
//!   sweeps every linked project's `.cursor/mcp.json` so Cursor keeps
//!   working without the user copying anything by hand. The new token is
//!   never returned over IPC: the panel surfaces only the URL and counts.

use std::sync::Arc;

use tauri::State;

use crate::error::AppError;
use crate::mcp::{auth, config, install};
use crate::projects::store::ProjectStore;

/// Status snapshot rendered by the Settings panel "Cursor integration"
/// section. Counts are derived live from the store on each call so the panel
/// stays in sync with project add / remove without a separate event.
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    /// Full URL clients connect to, e.g. `http://127.0.0.1:47821/mcp`.
    pub url: String,
    /// Loopback port the listener is bound to.
    pub port: u16,
    /// Number of imported projects that have a slug (proxy for "the Cursor
    /// MCP config has been written").
    pub linked_projects: u32,
}

/// Aggregate result of [`mcp_rotate_token`]: how many projects were touched
/// and how many of those failed to rewrite. Mirrors the relevant fields of
/// [`install::SweepReport`] without leaking the new token.
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RotateReport {
    /// Imported projects considered (i.e. slug present and repo on disk).
    pub linked_projects: u32,
    /// Subset of [`Self::linked_projects`] whose rewrite failed.
    pub failed: u32,
}

/// List snapshots for a project. Returned to the UI as a list of records;
/// the UI maps timestamps and labels for display and uses `id` to drive
/// rollback / delete.
#[tauri::command]
#[specta::specta]
pub async fn mcp_list_snapshots(
    store: State<'_, Arc<dyn ProjectStore>>,
    #[allow(
        non_snake_case,
        reason = "Tauri command params must be camelCase to match frontend"
    )]
    projectId: String,
) -> Result<Vec<crate::snapshots::SnapshotRecord>, AppError> {
    let id = crate::projects::ProjectId::new(&projectId).map_err(|e| AppError::InvalidInput {
        field: "projectId".into(),
        reason: e.to_string(),
    })?;
    store.list_snapshots(&id)
}

/// Delete a snapshot: remove the on-disk tree at
/// `~/.opnble/snapshots/<project_id>/<snapshot_id>` and the record from
/// `state.json`. Removal of the disk dir is best-effort: a missing dir is
/// not an error (snapshot may already be partially evicted).
#[tauri::command]
#[specta::specta]
pub async fn mcp_delete_snapshot(
    store: State<'_, Arc<dyn ProjectStore>>,
    #[allow(
        non_snake_case,
        reason = "Tauri command params must be camelCase to match frontend"
    )]
    snapshotId: String,
) -> Result<(), AppError> {
    let id = crate::snapshots::SnapshotId::new(snapshotId).map_err(|e| AppError::InvalidInput {
        field: "snapshotId".into(),
        reason: e.to_string(),
    })?;
    let record = store.get_snapshot(&id)?;
    let dir =
        crate::constants::paths::project_snapshot_dir(record.project_id.as_str(), id.as_str())?;
    if dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    store.remove_snapshot(&id)
}

/// Discard a forked project: force-stop its container, remove from the
/// store, and delete the repo tree on disk. Rejects when the target is not
/// a fork (`parent_project_id` is `None`). The non-fork case is the existing
/// delete-project flow which handles tunnels and pages; forks reuse none
/// of that today (forks do not share tunnels/pages with the parent).
#[tauri::command]
#[specta::specta]
pub async fn mcp_discard_fork(
    store: State<'_, Arc<dyn ProjectStore>>,
    orchestrator: State<'_, Arc<crate::projects::ProjectOrchestrator>>,
    #[allow(
        non_snake_case,
        reason = "Tauri command params must be camelCase to match frontend"
    )]
    forkId: String,
) -> Result<(), AppError> {
    let id = crate::projects::ProjectId::new(&forkId).map_err(|e| AppError::InvalidInput {
        field: "forkId".into(),
        reason: e.to_string(),
    })?;
    let project = store.get(&id)?;
    if project.parent_project_id.is_none() {
        return Err(AppError::InvalidInput {
            field: "forkId".into(),
            reason: "project is not a fork".into(),
        });
    }
    let _ = orchestrator.force_stop_and_remove(id.as_str()).await;
    store.remove(&id)?;
    if std::path::Path::new(&project.local_path).exists() {
        let _ = std::fs::remove_dir_all(&project.local_path);
    }
    Ok(())
}

/// Roll back a project's working tree to a snapshot. Delegates to the same
/// `rollback_project_impl` free function the MCP tool uses, so the frontend
/// rollback button and the MCP agent path share one code path. Requires a
/// live `AppHandle` because a running project must be stopped and restarted;
/// passes `Some(&app)` unconditionally.
#[tauri::command]
#[specta::specta]
pub async fn mcp_rollback_project(
    app: tauri::AppHandle,
    store: State<'_, Arc<dyn ProjectStore>>,
    orchestrator: State<'_, Arc<crate::projects::ProjectOrchestrator>>,
    #[allow(
        non_snake_case,
        reason = "Tauri command params must be camelCase to match frontend"
    )]
    slug: String,
    #[allow(
        non_snake_case,
        reason = "Tauri command params must be camelCase to match frontend"
    )]
    snapshotId: String,
) -> Result<crate::mcp::tools::types::RollbackView, AppError> {
    let args = crate::mcp::tools::types::RollbackArgs {
        slug,
        snapshot_id: snapshotId,
    };
    let snapshot_root = crate::constants::paths::snapshots_dir()?;
    let orch = crate::mcp::orch::LiveOrchestrator::new(Arc::clone(orchestrator.inner()));
    crate::mcp::tools::discovery::rollback_project_impl(
        store.inner().as_ref(),
        &orch,
        Some(&app),
        args,
        &snapshot_root,
    )
    .await
    .map_err(|mcp_err| AppError::Internal {
        reason: mcp_err.message.to_string(),
    })
}

/// Snapshot of the most recent MCP tool invocations recorded by the
/// in-memory ring buffer (cap 50, FIFO). Used by the Cursor-integration
/// settings panel to show users what host-side agents have been doing.
#[tauri::command]
#[specta::specta]
pub async fn mcp_recent_tool_invocations(
    ring: tauri::State<'_, Arc<crate::mcp::ring::ToolRing>>,
) -> Result<Vec<crate::mcp::ring::RingEntry>, AppError> {
    Ok(ring.snapshot())
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_status(store: State<'_, Arc<dyn ProjectStore>>) -> Result<McpStatus, AppError> {
    let projects = store.list()?;
    let linked = projects.iter().filter(|p| p.slug.is_some()).count();
    Ok(McpStatus {
        url: config::mcp_url(),
        port: config::PORT,
        linked_projects: u32::try_from(linked).unwrap_or(u32::MAX),
    })
}

/// Regenerate the local MCP bearer token, update the running listener's
/// in-memory copy, and rewrite every linked project's `.cursor/mcp.json`.
///
/// Three-step flow under one logical operation:
/// 1. [`auth::rotate_token`] writes the new token to `~/.opnble/mcp.json`
///    and updates the [`auth::SharedToken`] held by the listener's
///    `bearer_auth_layer` middleware. After this returns, the OLD token
///    is no longer accepted (security contract: scraped tokens stop
///    working at rotation, not at app restart).
/// 2. [`install::sweep_token`] walks the project list and rewrites each
///    `.cursor/mcp.json`.
/// 3. Each project's `.gitignore` is reaffirmed so a freshly rotated token
///    cannot land in source control even if the user removed the line.
///
/// Partial-failure contract: per-project sweep failures are tolerated and
/// reflected in `RotateReport.failed` so the panel can prompt for a manual
/// reconnect; the canonical token in `~/.opnble/` is already the new one
/// and the in-memory listener has already swapped over.
#[tauri::command]
#[specta::specta]
pub async fn mcp_rotate_token(
    store: State<'_, Arc<dyn ProjectStore>>,
    token: State<'_, auth::SharedToken>,
) -> Result<RotateReport, AppError> {
    let new_token = auth::rotate_token(token.inner()).await?;
    let report = install::sweep_token(store.inner().as_ref(), &new_token, config::PORT)?;
    Ok(RotateReport {
        linked_projects: u32::try_from(report.total).unwrap_or(u32::MAX),
        failed: u32::try_from(report.failed.len()).unwrap_or(u32::MAX),
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::projects::types::{Project, ProjectId};

    /// Tiny in-memory store mirroring the pattern in `mcp::install` tests.
    /// Mutating methods are unreachable from these tests so they return a
    /// clear `Internal` if anything ever calls them.
    struct MockStore {
        projects: Vec<Project>,
    }

    fn unused(method: &str) -> AppError {
        AppError::Internal {
            reason: format!("MockStore::{method} unused in mcp::commands tests"),
        }
    }

    impl ProjectStore for MockStore {
        fn list(&self) -> Result<Vec<Project>, AppError> {
            Ok(self.projects.clone())
        }
        fn get(&self, _id: &ProjectId) -> Result<Project, AppError> {
            Err(unused("get"))
        }
        fn add(&self, _project: Project) -> Result<(), AppError> {
            Err(unused("add"))
        }
        fn update(&self, _project: Project) -> Result<(), AppError> {
            Err(unused("update"))
        }
        fn remove(&self, _id: &ProjectId) -> Result<(), AppError> {
            Err(unused("remove"))
        }
        fn allocate_port(&self) -> Result<u16, AppError> {
            Err(unused("allocate_port"))
        }
        fn next_port(&self) -> Result<u16, AppError> {
            Err(unused("next_port"))
        }
        fn add_snapshot(&self, _record: crate::snapshots::SnapshotRecord) -> Result<(), AppError> {
            Err(unused("add_snapshot"))
        }
        fn list_snapshots(
            &self,
            _project_id: &ProjectId,
        ) -> Result<Vec<crate::snapshots::SnapshotRecord>, AppError> {
            Err(unused("list_snapshots"))
        }
        fn remove_snapshot(&self, _id: &crate::snapshots::SnapshotId) -> Result<(), AppError> {
            Err(unused("remove_snapshot"))
        }
        fn get_snapshot(
            &self,
            _id: &crate::snapshots::SnapshotId,
        ) -> Result<crate::snapshots::SnapshotRecord, AppError> {
            Err(unused("get_snapshot"))
        }
    }

    fn project_at(id: &str, slug: Option<&str>, repo: &Path) -> Project {
        let mut p = Project::new(
            ProjectId::new(id).expect("valid id"),
            id.to_string(),
            "https://example.com/repo".to_string(),
            repo.to_string_lossy().into_owned(),
            "main".to_string(),
        );
        p.slug = slug.map(str::to_string);
        p
    }

    /// Build the status payload the same way [`mcp_status`] does, but
    /// against an arbitrary store. Lets tests cover the counting logic
    /// without spinning up a Tauri runtime.
    fn status_for(store: &dyn ProjectStore) -> Result<McpStatus, AppError> {
        let projects = store.list()?;
        let linked = projects.iter().filter(|p| p.slug.is_some()).count();
        Ok(McpStatus {
            url: config::mcp_url(),
            port: config::PORT,
            linked_projects: u32::try_from(linked).unwrap_or(u32::MAX),
        })
    }

    #[test]
    fn status_counts_only_slugged_projects() {
        let dir = std::path::PathBuf::from("/tmp/opnble-mcp-status-test");
        let store = MockStore {
            projects: vec![
                project_at("p1", Some("alpha"), &dir),
                project_at("p2", None, &dir),
                project_at("p3", Some("beta"), &dir),
            ],
        };

        let status = status_for(&store).expect("status");
        assert_eq!(status.port, config::PORT);
        assert_eq!(status.url, config::mcp_url());
        assert_eq!(status.linked_projects, 2);
    }

    #[test]
    fn status_zero_when_no_projects() {
        let store = MockStore { projects: vec![] };
        let status = status_for(&store).expect("status");
        assert_eq!(status.linked_projects, 0);
    }

    #[test]
    fn status_zero_when_no_slugs() {
        let dir = std::path::PathBuf::from("/tmp/opnble-mcp-status-test");
        let store = MockStore {
            projects: vec![project_at("p1", None, &dir), project_at("p2", None, &dir)],
        };
        let status = status_for(&store).expect("status");
        assert_eq!(status.linked_projects, 0);
    }
}
