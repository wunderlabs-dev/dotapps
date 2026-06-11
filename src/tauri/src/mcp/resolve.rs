//! Slug -> Project lookup for MCP tools.
//!
//! Tool args carry a project's portable `slug` (e.g. `"my-blog"`), not the
//! machine-local `proj-{millis}` id. This indirection survives `git clone`
//! to another machine, where the same repo's slug should resolve to the
//! corresponding locally-imported project.
//!
//! `ProjectStore::get_by_slug` (added in Phase 0b) already does the lookup
//! and emits `AppError::NotFound { entity: "project", id: slug }` on miss;
//! we just keep this layer as a stable seam for Phase 2 tools to call.

use crate::error::AppError;
use crate::projects::store::ProjectStore;
use crate::projects::types::Project;

/// Resolve a project by its portable slug. Returns the same `NotFound` shape
/// the store produces so the error layer can map it to `PROJECT_NOT_FOUND`.
pub fn project_for_slug(store: &dyn ProjectStore, slug: &str) -> Result<Project, AppError> {
    store.get_by_slug(slug)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::types::{ProjectId, ProjectIntent, ProjectStatus};

    /// In-memory `ProjectStore` that ignores writes: enough to exercise the
    /// resolver without dragging in the full JSON-on-disk store.
    struct StubStore {
        projects: Vec<Project>,
    }

    impl ProjectStore for StubStore {
        fn list(&self) -> Result<Vec<Project>, AppError> {
            Ok(self.projects.clone())
        }
        fn get(&self, id: &ProjectId) -> Result<Project, AppError> {
            self.projects
                .iter()
                .find(|p| p.id == *id)
                .cloned()
                .ok_or_else(|| AppError::NotFound {
                    entity: "project".into(),
                    id: id.to_string(),
                })
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

    fn make_project(id: &str, slug: Option<&str>) -> Project {
        Project {
            id: ProjectId::new(id).expect("valid id"),
            name: id.to_string(),
            slug: slug.map(str::to_string),
            repo_url: "https://example.com/repo".into(),
            local_path: format!("/tmp/{id}"),
            status: ProjectStatus::Stopped,
            intent: ProjectIntent::Stop,
            port: None,
            branch: "main".into(),
            last_synced_at: 0,
            tunnel_id: None,
            tunnel_url: None,
            pages_url: None,
            installed: false,
            parent_project_id: None,
            forked_at: None,
        }
    }

    #[test]
    fn project_for_slug_hits_when_slug_matches() {
        let store = StubStore {
            projects: vec![make_project("proj-1", Some("foo"))],
        };
        let project = project_for_slug(&store, "foo").expect("hit");
        assert_eq!(project.id.as_str(), "proj-1");
    }

    #[test]
    fn project_for_slug_returns_not_found_when_slug_missing() {
        let store = StubStore {
            projects: vec![make_project("proj-1", Some("foo"))],
        };
        let err = project_for_slug(&store, "bar").expect_err("miss");
        assert!(
            matches!(&err, AppError::NotFound { entity, id } if entity == "project" && id == "bar"),
            "expected NotFound, got {err:?}"
        );
    }

    #[test]
    fn project_for_slug_skips_projects_without_slug() {
        let store = StubStore {
            projects: vec![
                make_project("proj-1", None),
                make_project("proj-2", Some("foo")),
            ],
        };
        let hit = project_for_slug(&store, "foo").expect("hit");
        assert_eq!(hit.id.as_str(), "proj-2");

        assert!(project_for_slug(&store, "").is_err());
    }
}
