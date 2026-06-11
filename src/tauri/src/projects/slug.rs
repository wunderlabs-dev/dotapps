//! Portable slug derivation for projects.
//!
//! A slug is a stable, kebab-case identity derived from the repository URL.
//! Unlike [`crate::projects::types::ProjectId`] (which is `proj-{unix_millis}`
//! from the importing machine), a slug survives `git clone` to a different
//! machine, so it can be committed into the repo's future `.opnble` marker
//! file and used by MCP tools to resolve a project across machines.

use std::collections::HashSet;

use crate::error::AppError;
use crate::projects::store::ProjectStore;

/// Fallback slug used when [`derive_slug`] is given a URL whose last path
/// segment contains no ASCII alphanumeric characters (for example
/// `https://example.com/`).
const FALLBACK_SLUG: &str = "project";

/// Derive a portable slug from the given `repo_url`.
///
/// Strategy: take the last `/`-separated segment, strip a trailing `.git`,
/// lowercase, replace any run of non-alphanumeric ASCII with a single `-`,
/// and trim leading and trailing `-`. The result is non-empty, contains
/// only ASCII alphanumeric characters and `-`, and never has consecutive
/// `-` runs. If the input yields an empty result the fallback `"project"`
/// is returned.
pub fn derive_slug(repo_url: &str) -> String {
    // `rsplit` on a non-empty pattern always yields at least one element,
    // even for the empty string ("" rsplits to [""]), so this is safe.
    let last_segment = repo_url.rsplit('/').next().unwrap_or("");
    let stripped = last_segment.strip_suffix(".git").unwrap_or(last_segment);

    let mut slug = String::with_capacity(stripped.len());
    for ch in stripped.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }

    let trimmed = slug.trim_end_matches('-');
    if trimmed.is_empty() {
        FALLBACK_SLUG.to_string()
    } else {
        trimmed.to_string()
    }
}

/// Derive a slug for `repo_url` that does not collide with any existing
/// slug in `store`.
///
/// If [`derive_slug`]'s base output is already taken, suffix with `-2`,
/// `-3`, ... until a free slot is found. Projects without a slug
/// (`slug: None`) are ignored when looking for collisions.
pub fn derive_slug_unique(store: &dyn ProjectStore, repo_url: &str) -> Result<String, AppError> {
    let taken: HashSet<String> = store.list()?.into_iter().filter_map(|p| p.slug).collect();
    derive_slug_unique_excluding(&taken, repo_url)
}

/// Derive a slug for `repo_url` that does not collide with any slug in `taken`.
///
/// Same suffixing strategy as [`derive_slug_unique`], but driven by an
/// explicit set so callers that already hold the project list (for example a
/// load-time backfill that assigns several slugs in one pass) can grow the set
/// as they go without re-reading a store.
pub fn derive_slug_unique_excluding(
    taken: &HashSet<String>,
    repo_url: &str,
) -> Result<String, AppError> {
    let base = derive_slug(repo_url);
    if !taken.contains(&base) {
        return Ok(base);
    }

    let mut counter: u32 = 2;
    loop {
        let candidate = format!("{base}-{counter}");
        if !taken.contains(&candidate) {
            return Ok(candidate);
        }
        counter = counter.checked_add(1).ok_or_else(|| AppError::Internal {
            reason: format!("cannot derive unique slug for '{base}': counter overflow"),
        })?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::types::{Project, ProjectId};

    #[test]
    fn derive_slug_strips_dot_git() {
        assert_eq!(
            derive_slug("https://github.com/owner/my-blog.git"),
            "my-blog"
        );
    }

    #[test]
    fn derive_slug_lowercases_camelcase() {
        assert_eq!(derive_slug("https://github.com/owner/MyBlog.git"), "myblog");
    }

    #[test]
    fn derive_slug_replaces_underscore_with_dash() {
        assert_eq!(derive_slug("https://github.com/owner/my_blog"), "my-blog");
    }

    #[test]
    fn derive_slug_collapses_dots_in_ssh_url() {
        assert_eq!(
            derive_slug("git@github.com:owner/my.cool.repo.git"),
            "my-cool-repo"
        );
    }

    #[test]
    fn derive_slug_falls_back_when_last_segment_is_empty() {
        assert_eq!(derive_slug("https://example.com/"), "project");
    }

    #[test]
    fn derive_slug_collapses_runs_of_separators() {
        assert_eq!(derive_slug("https://example.com/a---b___c"), "a-b-c");
    }

    #[test]
    fn derive_slug_trims_leading_and_trailing_separators() {
        assert_eq!(
            derive_slug("https://example.com/--weird--name--"),
            "weird-name"
        );
    }

    /// Tiny in-memory `ProjectStore` for testing collision behaviour.
    ///
    /// Holds a plain `Vec<Project>` because `derive_slug_unique` only ever
    /// calls [`ProjectStore::list`], which returns a clone. The mutating
    /// methods on the trait are unreachable from these tests and return
    /// `Internal` errors so a future test that misuses the mock fails
    /// loudly instead of silently.
    struct MockStore {
        projects: Vec<Project>,
    }

    impl MockStore {
        fn new(slugs: &[Option<&str>]) -> Self {
            let projects = slugs
                .iter()
                .enumerate()
                .map(|(i, slug)| {
                    let id = ProjectId::new(format!("proj-{i}")).expect("valid generated id");
                    let mut project = Project::new(
                        id,
                        format!("Project {i}"),
                        "https://example.com/repo".to_string(),
                        format!("/tmp/proj-{i}"),
                        "main".to_string(),
                    );
                    project.slug = slug.map(str::to_string);
                    project
                })
                .collect();
            Self { projects }
        }
    }

    fn unused(method: &str) -> AppError {
        AppError::Internal {
            reason: format!("MockStore::{method} is unused in slug tests"),
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
    fn derive_slug_unique_returns_base_when_no_collision() {
        let store = MockStore::new(&[]);
        let slug = derive_slug_unique(&store, "https://github.com/owner/my-blog.git")
            .expect("no error expected");
        assert_eq!(slug, "my-blog");
    }

    #[test]
    fn derive_slug_unique_ignores_projects_without_slug() {
        let store = MockStore::new(&[None, None]);
        let slug = derive_slug_unique(&store, "https://github.com/owner/my-blog.git")
            .expect("no error expected");
        assert_eq!(slug, "my-blog");
    }

    #[test]
    fn derive_slug_unique_bumps_suffix_on_collisions() {
        let store = MockStore::new(&[Some("my-blog"), Some("my-blog-2")]);
        let slug = derive_slug_unique(&store, "https://github.com/owner/my-blog.git")
            .expect("no error expected");
        assert_eq!(slug, "my-blog-3");
    }

    #[test]
    fn derive_slug_unique_starts_suffix_at_two() {
        let store = MockStore::new(&[Some("my-blog")]);
        let slug = derive_slug_unique(&store, "https://github.com/owner/my-blog.git")
            .expect("no error expected");
        assert_eq!(slug, "my-blog-2");
    }
}
