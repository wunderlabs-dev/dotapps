//! Project import service - handles cloning and project creation workflow

use std::path::Path;
use std::sync::Arc;

use crate::error::AppError;
use crate::infrastructure::git::GitOps;
use crate::mcp;
use crate::projects::marker::{write_marker, Marker};
use crate::projects::slug::derive_slug_unique;
use crate::projects::{
    store::ProjectStore,
    types::{Project, ProjectId},
};

/// Imports projects from git repositories
pub struct ProjectImporter {
    project: Arc<dyn ProjectStore>,
    git: Arc<dyn GitOps>,
}

impl ProjectImporter {
    pub fn new(project: Arc<dyn ProjectStore>, git: Arc<dyn GitOps>) -> Self {
        Self { project, git }
    }

    /// Import a project from a git URL.
    ///
    /// Clones the repository, derives a portable slug, writes the
    /// committed `.opnble` marker, best-effort installs `.cursor/mcp.json`
    /// for Cursor's project-scoped MCP, then persists the project record.
    pub fn import(&self, url: &str, name: &str, token: Option<&str>) -> Result<Project, AppError> {
        let id = ProjectId::generate();
        let path = id.repo_path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let clone_result = self.git.clone_repo(url, &path, token)?;
        let local_path = clone_result.local_path;
        let repo_path = Path::new(&local_path);
        let slug = after_clone_install(self.project.as_ref(), url, repo_path)?;

        let project = Project::new(
            id,
            name.to_string(),
            url.to_string(),
            local_path,
            clone_result.default_branch,
        )
        .with_slug(slug);

        self.project.add(project.clone())?;

        Ok(project)
    }
}

/// Run the post-clone wiring shared by every import path: derive a
/// portable slug, write the committed `.opnble` marker, and best-effort
/// install `.cursor/mcp.json` plus its `.gitignore` line.
///
/// Returns the slug so the caller can attach it to the new
/// [`Project`] record. MCP install failures are logged via `tracing::warn!`
/// and swallowed so a missing token or unwritable `.cursor/` directory
/// does not block import.
pub(crate) fn after_clone_install(
    store: &dyn ProjectStore,
    repo_url: &str,
    repo_path: &Path,
) -> Result<String, AppError> {
    let slug = derive_slug_unique(store, repo_url)?;
    write_marker(repo_path, &Marker::new(slug.clone(), repo_url.to_string()))?;
    mcp::install::install_project_entry(repo_path, &slug);
    Ok(slug)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// In-memory store sufficient for `after_clone_install`'s read-only
    /// contract: `derive_slug_unique` only calls `list` and the helper
    /// never mutates the store. The mutating trait methods return errors
    /// so accidental misuse fails loudly.
    struct MockStore {
        projects: Vec<Project>,
    }

    impl MockStore {
        fn empty() -> Self {
            Self {
                projects: Vec::new(),
            }
        }

        fn with_slugs(slugs: &[&str]) -> Self {
            let projects = slugs
                .iter()
                .enumerate()
                .map(|(i, slug)| {
                    let id = ProjectId::new(format!("proj-{i}")).expect("valid id");
                    Project::new(
                        id,
                        format!("Project {i}"),
                        "https://example.com/repo".to_string(),
                        format!("/tmp/proj-{i}"),
                        "main".to_string(),
                    )
                    .with_slug((*slug).to_string())
                })
                .collect();
            Self { projects }
        }
    }

    fn unused(method: &str) -> AppError {
        AppError::Internal {
            reason: format!("MockStore::{method} unused in importer tests"),
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

    #[test]
    fn after_clone_install_writes_marker_with_url_derived_slug() {
        let dir = TempDir::new().expect("tempdir");
        let store = MockStore::empty();
        let url = "https://github.com/owner/my-blog.git";
        let slug = after_clone_install(&store, url, dir.path()).expect("ok");
        assert_eq!(slug, "my-blog");

        let marker_path = dir.path().join(".opnble");
        assert!(marker_path.exists(), "marker should be written");
        let body = std::fs::read_to_string(&marker_path).expect("read marker");
        assert!(body.contains("\"slug\": \"my-blog\""));
        assert!(body.contains("\"repo\": \"https://github.com/owner/my-blog.git\""));
    }

    #[test]
    fn after_clone_install_returns_unique_slug_against_store() {
        let dir = TempDir::new().expect("tempdir");
        let store = MockStore::with_slugs(&["my-blog"]);
        let url = "https://github.com/owner/my-blog.git";
        let slug = after_clone_install(&store, url, dir.path()).expect("ok");
        assert_eq!(slug, "my-blog-2");
    }

    #[test]
    fn after_clone_install_swallows_mcp_failures() {
        // No token file in this isolated tempdir, so the MCP install path
        // logs and returns; the helper itself must still succeed.
        let dir = TempDir::new().expect("tempdir");
        let store = MockStore::empty();
        let slug = after_clone_install(&store, "https://example.com/owner/foo", dir.path())
            .expect("helper must not surface mcp failures");
        assert_eq!(slug, "foo");
    }
}
