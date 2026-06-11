//! Git operations via libgit2

use std::path::Path;

use git2::{
    BranchType, Cred, FetchOptions, IndexAddOption, PushOptions, RemoteCallbacks, Repository,
    Signature,
};
use serde::Serialize;

use crate::error::AppError;

/// Result of a clone operation
#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CloneResult {
    pub local_path: String,
    pub default_branch: String,
}

/// Result of `commit_auto`: distinguishes committed vs no-op
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommitAutoResult {
    Committed,
    NoOp,
}

/// Push operation outcome.
///
/// **Caller contract:** Always match on the variant. `Failed` is returned inside
/// `Ok(...)` (local-first semantics): the operation was attempted but the remote
/// rejected or the network failed. Use `Err` only for pre-conditions (detached
/// HEAD, repo not found). Callers that ignore `Failed` will incorrectly treat
/// push failures as success.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PushOutcome {
    Pushed,
    NothingToPush,
    /// Remote push failed (auth, network, non-fast-forward). See module docs.
    Failed(String),
}

/// Sync status relative to remote tracking branch
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepoSyncStatus {
    Clean,
    Ahead,
    Behind,
    Diverged,
}

/// Porcelain working-tree state for a repository.
///
/// `branch` is the human-readable branch name. For unborn branches (freshly
/// initialised repo, no commits yet) it is the configured initial branch (for
/// example `main`). For detached HEAD it is a synthesized informational string
/// of the form `(HEAD detached at <short-sha>)` so MCP `git_status` callers can
/// still introspect a worktree they cannot push or pull from. Use
/// `current_commit_sha` if you need the canonical OID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoStatus {
    pub branch: String,
    pub ahead: usize,
    pub behind: usize,
    pub dirty: bool,
    pub files: Vec<FileStatus>,
}

/// One changed path in `RepoStatus::files`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileStatus {
    pub path: String,
    pub status: FileChange,
}

/// Classification of a single path's git status, collapsed across index +
/// worktree flags. When a file has multiple libgit2 status bits set we pick the
/// most specific: `Renamed > Deleted > Added > Modified > Untracked`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FileChange {
    Added,
    Modified,
    Deleted,
    Untracked,
    Renamed,
}

impl FileChange {
    /// Stable camelCase discriminator matching the `serde` wire shape.
    ///
    /// Returned as `&'static str` so MCP `git_status` can build a
    /// `FileStatusView` without going through `serde_json::to_value` for
    /// every file in a repo's status.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
            Self::Untracked => "untracked",
            Self::Renamed => "renamed",
        }
    }
}

/// Default commit message for auto-sync
pub fn default_commit_message() -> &'static str {
    "Updated project files"
}

/// Trait for git operations
pub trait GitOps: Send + Sync {
    /// Clone a repository
    fn clone_repo(
        &self,
        url: &str,
        target_path: &Path,
        token: Option<&str>,
    ) -> Result<CloneResult, AppError>;

    /// Pull latest changes
    fn pull(&self, repo_path: &Path, token: Option<&str>) -> Result<(), AppError>;

    /// Checkout a branch.
    ///
    /// When `create == true`, create the branch from current HEAD before
    /// checking it out. Returns `AlreadyExists` if the branch already exists.
    /// When `create == false`, the branch must already exist.
    fn checkout(&self, repo_path: &Path, branch: &str, create: bool) -> Result<(), AppError>;

    /// List local branches
    fn list_branches(&self, repo_path: &Path) -> Result<Vec<String>, AppError>;

    /// Porcelain working-tree status (branch, ahead/behind, changed files).
    ///
    /// Tolerates detached HEAD and unborn branches: see `RepoStatus::branch`.
    /// Branches without an upstream report `ahead = 0, behind = 0` rather than
    /// erroring.
    fn status(&self, repo_path: &Path) -> Result<RepoStatus, AppError>;

    /// Register a new named remote pointing at `url`.
    ///
    /// libgit2 rejects duplicate remote names; that bubbles up as `GitFailed`.
    fn add_remote(&self, repo_path: &Path, name: &str, url: &str) -> Result<(), AppError>;

    /// Whether a remote with `name` is configured for the repository.
    ///
    /// Used by the MCP `github_push` tool's `create_repo_if_missing` flow to
    /// decide whether to create a GitHub repo before pushing. Returns
    /// `Ok(false)` only when libgit2 reports the remote as missing
    /// (`ErrorCode::NotFound`); other failures (corrupt config, IO) propagate
    /// as `GitFailed` so the caller does not silently skip the push.
    fn has_remote(&self, repo_path: &Path, name: &str) -> Result<bool, AppError>;

    /// URL of an existing remote, or `None` if libgit2 cannot decode it as
    /// UTF-8 (no real-world remote URL is non-UTF8, but the API tolerates
    /// it). Returns `GitFailed` when the remote does not exist.
    fn remote_url(&self, repo_path: &Path, name: &str) -> Result<Option<String>, AppError>;

    /// SHA of the commit HEAD currently points at, as a 40-character hex string.
    fn current_commit_sha(&self, repo_path: &Path) -> Result<String, AppError>;

    /// Commit staged changes with message; returns Committed or `NoOp` if nothing to commit
    fn commit_auto(&self, repo_path: &Path, message: &str) -> Result<CommitAutoResult, AppError>;

    /// Push current branch to origin; requires token for authenticated remotes
    fn push_current_branch(
        &self,
        repo_path: &Path,
        token: Option<&str>,
    ) -> Result<PushOutcome, AppError>;

    /// Sync status relative to remote tracking branch (ahead/behind/diverged/clean)
    fn repo_sync_status(&self, repo_path: &Path) -> Result<RepoSyncStatus, AppError>;

    /// Use for publishing build artifacts to GitHub Pages.
    ///
    /// Builds a tree from `source_dir`, creates a parentless commit,
    /// and force-pushes to `origin/<branch>`.
    fn push_orphan_branch(
        &self,
        repo_path: &Path,
        branch: &str,
        source_dir: &Path,
        message: &str,
        token: Option<&str>,
    ) -> Result<PushOutcome, AppError>;
}

/// libgit2-based implementation
pub struct LibGitClient;

impl LibGitClient {
    pub fn new() -> Self {
        Self
    }

    fn create_fetch_options(token: Option<&str>) -> FetchOptions<'_> {
        let mut callbacks = RemoteCallbacks::new();

        if let Some(token) = token {
            let token = token.to_owned();
            callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
                Cred::userpass_plaintext("oauth2", &token)
            });
        }

        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);
        fetch_opts
    }

    fn create_push_options(token: Option<&str>) -> PushOptions<'_> {
        let mut callbacks = RemoteCallbacks::new();

        if let Some(token) = token {
            let token = token.to_owned();
            callbacks.credentials(move |_url, _username, _allowed| {
                Cred::userpass_plaintext("oauth2", &token)
            });
        }

        let mut push_opts = PushOptions::new();
        push_opts.remote_callbacks(callbacks);
        push_opts
    }

    fn ensure_head_branch_name<'repo>(
        head: &'repo git2::Reference<'repo>,
    ) -> Result<&'repo str, AppError> {
        if !head.is_branch() {
            return Err(AppError::GitFailed {
                reason: "cannot sync: HEAD is detached. checkout a branch first.".to_string(),
            });
        }
        head.shorthand().ok_or_else(|| AppError::GitFailed {
            reason: "cannot resolve HEAD shorthand".to_string(),
        })
    }

    /// Resolve upstream tracking ref for comparison/merge.
    /// Prefers `branch.upstream()` when configured; falls back to `origin/<branch>`.
    fn resolve_upstream_ref(repo: &Repository, branch_name: &str) -> String {
        if let Ok(branch) = repo.find_branch(branch_name, BranchType::Local) {
            if let Ok(upstream) = branch.upstream() {
                if let Some(name) = upstream.get().name() {
                    return name.to_string();
                }
            }
        }
        format!("refs/remotes/origin/{branch_name}")
    }
}

impl Default for LibGitClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Recursively build a git tree from a filesystem directory.
///
/// Each file becomes a blob, each subdirectory becomes a subtree.
/// Hidden files (starting with `.`) are skipped.
fn build_tree_from_dir(repo: &Repository, dir: &Path) -> Result<git2::Oid, AppError> {
    let mut builder = repo.treebuilder(None)?;

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| AppError::StorageFailed {
            reason: format!("cannot read build output directory {}: {e}", dir.display()),
        })?
        .filter_map(Result::ok)
        .collect();

    // Sort for deterministic tree hashes
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let name = entry.file_name();
        let name_str = name.to_str().ok_or_else(|| AppError::StorageFailed {
            reason: format!("cannot read filename: {}", entry.path().display()),
        })?;

        // Skip hidden files/dirs
        if name_str.starts_with('.') {
            continue;
        }

        let file_type = entry.file_type().map_err(|e| AppError::StorageFailed {
            reason: format!("cannot read file type for {}: {e}", entry.path().display()),
        })?;

        if file_type.is_file() {
            let content = std::fs::read(entry.path()).map_err(|e| AppError::StorageFailed {
                reason: format!("cannot read {}: {e}", entry.path().display()),
            })?;
            let blob_oid = repo.blob(&content)?;
            builder.insert(name_str, blob_oid, 0o100_644)?;
        } else if file_type.is_dir() {
            let subtree_oid = build_tree_from_dir(repo, &entry.path())?;
            builder.insert(name_str, subtree_oid, 0o040_000)?;
        }
        // Skip symlinks and other special files
    }

    let oid = builder.write()?;
    Ok(oid)
}

/// Resolve the user-facing branch label for `RepoStatus::branch`.
///
/// Handles three cases: normal branch checkout, detached HEAD (returns a
/// synthesized informational name so MCP `git_status` callers still see useful
/// data), and unborn HEAD on a freshly initialised repo (returns the symbolic
/// target, for example `main`).
fn current_branch_label(repo: &Repository) -> Result<String, AppError> {
    match repo.head() {
        Ok(head) => {
            if head.is_branch() {
                return head
                    .shorthand()
                    .map(str::to_string)
                    .ok_or_else(|| AppError::GitFailed {
                        reason: "cannot resolve HEAD shorthand".to_string(),
                    });
            }
            let oid = head.target().ok_or_else(|| AppError::GitFailed {
                reason: "cannot resolve HEAD target".to_string(),
            })?;
            let sha = oid.to_string();
            let short = &sha[..sha.len().min(7)];
            Ok(format!("(HEAD detached at {short})"))
        }
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
            let head_ref = repo.find_reference("HEAD")?;
            let target = head_ref
                .symbolic_target()
                .ok_or_else(|| AppError::GitFailed {
                    reason: "cannot resolve symbolic HEAD target".to_string(),
                })?;
            Ok(target
                .strip_prefix("refs/heads/")
                .unwrap_or(target)
                .to_string())
        }
        Err(e) => Err(e.into()),
    }
}

/// Ahead/behind counts vs the upstream tracking ref.
///
/// Returns `(0, 0)` when there is no comparison to make: detached HEAD, unborn
/// branch, or branch with no upstream configured. Only `graph_ahead_behind`
/// failures propagate as `GitFailed`.
fn compute_ahead_behind(repo: &Repository) -> Result<(usize, usize), AppError> {
    let Ok(head) = repo.head() else {
        return Ok((0, 0));
    };
    if !head.is_branch() {
        return Ok((0, 0));
    }
    let Some(local_oid) = head.target() else {
        return Ok((0, 0));
    };
    let Some(branch_name) = head.shorthand() else {
        return Ok((0, 0));
    };
    let upstream_ref = LibGitClient::resolve_upstream_ref(repo, branch_name);
    let Ok(upstream) = repo.find_reference(&upstream_ref) else {
        return Ok((0, 0));
    };
    let Some(remote_oid) = upstream.target() else {
        return Ok((0, 0));
    };
    let (ahead, behind) = repo.graph_ahead_behind(local_oid, remote_oid)?;
    Ok((ahead, behind))
}

/// Collapse libgit2's bitflag status into the porcelain `FileChange` enum.
///
/// Priority order when multiple bits are set: `Renamed > Deleted > Added >
/// Modified > Untracked`. Returns `None` for status combinations we do not
/// surface (clean entries, ignored, conflict-only) so the caller can skip them.
fn classify_file_status(status: git2::Status) -> Option<FileChange> {
    use git2::Status;
    let renamed = Status::INDEX_RENAMED | Status::WT_RENAMED;
    let deleted = Status::INDEX_DELETED | Status::WT_DELETED;
    let modified = Status::INDEX_MODIFIED | Status::WT_MODIFIED;
    if status.intersects(renamed) {
        Some(FileChange::Renamed)
    } else if status.intersects(deleted) {
        Some(FileChange::Deleted)
    } else if status.intersects(Status::INDEX_NEW) {
        Some(FileChange::Added)
    } else if status.intersects(modified) {
        Some(FileChange::Modified)
    } else if status.intersects(Status::WT_NEW) {
        Some(FileChange::Untracked)
    } else {
        None
    }
}

impl GitOps for LibGitClient {
    fn clone_repo(
        &self,
        url: &str,
        target_path: &Path,
        token: Option<&str>,
    ) -> Result<CloneResult, AppError> {
        let fetch_opts = Self::create_fetch_options(token);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_opts);

        let repo = builder.clone(url, target_path)?;

        let head = repo.head()?;

        let branch_name = head.shorthand().unwrap_or("main").to_string();

        Ok(CloneResult {
            local_path: target_path.to_string_lossy().to_string(),
            default_branch: branch_name,
        })
    }

    fn pull(&self, repo_path: &Path, token: Option<&str>) -> Result<(), AppError> {
        let repo = Repository::open(repo_path)?;

        // Reject detached HEAD (same branch safety as push/status).
        let head = repo.head()?;
        let branch_name = Self::ensure_head_branch_name(&head)?;

        let mut remote = repo.find_remote("origin")?;

        let mut fetch_opts = Self::create_fetch_options(token);

        remote.fetch(
            &["refs/heads/*:refs/remotes/origin/*"],
            Some(&mut fetch_opts),
            None,
        )?;

        let upstream_ref = Self::resolve_upstream_ref(&repo, branch_name);
        let fetch_head = repo.find_reference(&upstream_ref)?;

        let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;

        let (analysis, _) = repo.merge_analysis(&[&fetch_commit])?;

        if analysis.is_up_to_date() {
            return Ok(());
        }

        if analysis.is_fast_forward() {
            let refname = format!("refs/heads/{branch_name}");
            let mut reference = repo.find_reference(&refname)?;

            reference.set_target(fetch_commit.id(), "Fast-forward")?;

            repo.set_head(&refname)?;

            // Use a safe checkout so pull never discards local uncommitted changes.
            let mut checkout = git2::build::CheckoutBuilder::default();
            repo.checkout_head(Some(&mut checkout))?;

            return Ok(());
        }

        Err(AppError::GitFailed {
            reason: "cannot fast-forward, resolve conflicts manually".to_string(),
        })
    }

    fn checkout(&self, repo_path: &Path, branch: &str, create: bool) -> Result<(), AppError> {
        let repo = Repository::open(repo_path)?;

        if create {
            if repo.find_branch(branch, BranchType::Local).is_ok() {
                return Err(AppError::AlreadyExists {
                    entity: "branch".into(),
                    id: branch.into(),
                });
            }
            let head_commit = repo.head()?.peel_to_commit()?;
            repo.branch(branch, &head_commit, false)?;
        }

        let (object, reference) = repo.revparse_ext(branch)?;

        repo.checkout_tree(&object, None)?;

        match reference {
            Some(r) => repo.set_head(r.name().unwrap_or("HEAD")),
            None => repo.set_head_detached(object.id()),
        }?;

        Ok(())
    }

    fn list_branches(&self, repo_path: &Path) -> Result<Vec<String>, AppError> {
        let repo = Repository::open(repo_path)?;

        let branches = repo.branches(Some(git2::BranchType::Local))?;

        let mut result = Vec::new();
        for branch in branches {
            let (branch, _) = branch?;
            if let Some(name) = branch.name()? {
                result.push(name.to_string());
            }
        }

        Ok(result)
    }

    fn commit_auto(&self, repo_path: &Path, message: &str) -> Result<CommitAutoResult, AppError> {
        let repo = Repository::open(repo_path)?;

        let mut index = repo.index()?;

        index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
        index.update_all(["*"].iter(), None)?;

        index.write()?;

        let tree_id = index.write_tree()?;

        let tree = repo.find_tree(tree_id)?;

        let Ok(head) = repo.head() else {
            let sig = Signature::now("Opnble", "opnble@localhost")?;
            repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[])?;
            return Ok(CommitAutoResult::Committed);
        };

        let parent = repo.find_commit(head.target().ok_or_else(|| AppError::GitFailed {
            reason: "cannot resolve HEAD target".to_string(),
        })?)?;

        let parent_tree = parent.tree()?;
        let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;

        let stats = diff.stats()?;

        if stats.files_changed() == 0 {
            return Ok(CommitAutoResult::NoOp);
        }

        let sig = Signature::now("Opnble", "opnble@localhost")?;

        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])?;

        Ok(CommitAutoResult::Committed)
    }

    fn push_current_branch(
        &self,
        repo_path: &Path,
        token: Option<&str>,
    ) -> Result<PushOutcome, AppError> {
        let repo = Repository::open(repo_path)?;

        let head = repo.head()?;

        let branch_name = Self::ensure_head_branch_name(&head)?;

        let local_oid = head.target().ok_or_else(|| AppError::GitFailed {
            reason: "cannot resolve HEAD target".to_string(),
        })?;

        let upstream_ref = Self::resolve_upstream_ref(&repo, branch_name);
        if let Ok(upstream) = repo.find_reference(&upstream_ref) {
            if let Some(remote_oid) = upstream.target() {
                if local_oid == remote_oid {
                    return Ok(PushOutcome::NothingToPush);
                }
            }
        }

        let branch_ref = format!("refs/heads/{branch_name}");
        let refspec = format!("{branch_ref}:{branch_ref}");

        let mut remote = repo.find_remote("origin")?;

        let mut push_opts = Self::create_push_options(token);

        // Local-first: return Failed inside Ok so callers can distinguish
        // "push attempted but remote rejected" from "could not attempt" (Err).
        match remote.push(&[refspec.as_str()], Some(&mut push_opts)) {
            Ok(()) => Ok(PushOutcome::Pushed),
            Err(e) => Ok(PushOutcome::Failed(e.message().to_string())),
        }
    }

    fn repo_sync_status(&self, repo_path: &Path) -> Result<RepoSyncStatus, AppError> {
        let repo = Repository::open(repo_path)?;

        let head = repo.head()?;

        let branch_name = Self::ensure_head_branch_name(&head)?;

        let upstream_ref = Self::resolve_upstream_ref(&repo, branch_name);
        let Ok(upstream) = repo.find_reference(&upstream_ref) else {
            return Ok(RepoSyncStatus::Ahead);
        };

        let local_oid = head.target().ok_or_else(|| AppError::GitFailed {
            reason: "cannot resolve HEAD target".to_string(),
        })?;
        let remote_oid = upstream.target().ok_or_else(|| AppError::GitFailed {
            reason: "cannot resolve upstream target".to_string(),
        })?;

        if local_oid == remote_oid {
            return Ok(RepoSyncStatus::Clean);
        }

        let (ahead, behind) = repo.graph_ahead_behind(local_oid, remote_oid)?;

        Ok(match (ahead, behind) {
            (a, 0) if a > 0 => RepoSyncStatus::Ahead,
            (0, b) if b > 0 => RepoSyncStatus::Behind,
            _ => RepoSyncStatus::Diverged,
        })
    }

    fn status(&self, repo_path: &Path) -> Result<RepoStatus, AppError> {
        let repo = Repository::open(repo_path)?;

        let branch = current_branch_label(&repo)?;

        let mut status_opts = git2::StatusOptions::new();
        status_opts
            .include_untracked(true)
            .renames_head_to_index(true);

        let statuses = repo.statuses(Some(&mut status_opts))?;

        let mut files = Vec::with_capacity(statuses.len());
        for entry in statuses.iter() {
            let Some(path) = entry.path() else { continue };
            let Some(change) = classify_file_status(entry.status()) else {
                continue;
            };
            files.push(FileStatus {
                path: path.to_string(),
                status: change,
            });
        }

        let (ahead, behind) = compute_ahead_behind(&repo)?;
        let dirty = !files.is_empty();

        Ok(RepoStatus {
            branch,
            ahead,
            behind,
            dirty,
            files,
        })
    }

    fn add_remote(&self, repo_path: &Path, name: &str, url: &str) -> Result<(), AppError> {
        let repo = Repository::open(repo_path)?;
        repo.remote(name, url)?;
        Ok(())
    }

    fn has_remote(&self, repo_path: &Path, name: &str) -> Result<bool, AppError> {
        let repo = Repository::open(repo_path)?;
        let result = repo.find_remote(name);
        match result {
            Ok(_) => Ok(true),
            // libgit2 returns NotFound when the remote does not exist;
            // anything else (config corruption, IO) is a real GitFailed.
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    fn remote_url(&self, repo_path: &Path, name: &str) -> Result<Option<String>, AppError> {
        let repo = Repository::open(repo_path)?;
        let remote = repo.find_remote(name)?;
        Ok(remote.url().map(str::to_string))
    }

    fn current_commit_sha(&self, repo_path: &Path) -> Result<String, AppError> {
        let repo = Repository::open(repo_path)?;
        let oid = repo.head()?.peel_to_commit()?.id();
        Ok(oid.to_string())
    }

    fn push_orphan_branch(
        &self,
        repo_path: &Path,
        branch: &str,
        source_dir: &Path,
        message: &str,
        token: Option<&str>,
    ) -> Result<PushOutcome, AppError> {
        if !source_dir.is_dir() {
            return Err(crate::publish::PublishError::BuildOutputMissing {
                expected: source_dir.display().to_string(),
            }
            .into());
        }

        let repo = Repository::open(repo_path)?;

        // Build tree from the source directory
        let tree_oid = build_tree_from_dir(&repo, source_dir)?;
        let tree = repo.find_tree(tree_oid)?;

        // Create orphan commit (no parents)
        let sig = Signature::now("Opnble", "opnble@localhost")?;
        let commit_oid = repo.commit(None, &sig, &sig, message, &tree, &[])?;

        // Point the branch ref at the new commit (create or force-update)
        let branch_ref = format!("refs/heads/{branch}");
        repo.reference(&branch_ref, commit_oid, true, "publish to gh-pages")?;

        // Force-push the branch
        let refspec = format!("+{branch_ref}:{branch_ref}");
        let mut remote = repo.find_remote("origin")?;

        let mut push_opts = Self::create_push_options(token);

        match remote.push(&[refspec.as_str()], Some(&mut push_opts)) {
            Ok(()) => Ok(PushOutcome::Pushed),
            Err(e) => Ok(PushOutcome::Failed(e.message().to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_git_ops_trait_is_object_safe() {
        fn _accepts_dyn(_: &dyn GitOps) {}
    }

    #[test]
    fn test_default_commit_message() {
        assert_eq!(default_commit_message(), "Updated project files");
    }

    #[test]
    fn test_commit_auto_result_variants() {
        assert_eq!(CommitAutoResult::Committed, CommitAutoResult::Committed);
        assert_eq!(CommitAutoResult::NoOp, CommitAutoResult::NoOp);
        assert_ne!(CommitAutoResult::Committed, CommitAutoResult::NoOp);
    }

    #[test]
    fn test_push_outcome_variants() {
        assert_eq!(PushOutcome::Pushed, PushOutcome::Pushed);
        assert_eq!(PushOutcome::NothingToPush, PushOutcome::NothingToPush);
        assert_ne!(PushOutcome::Pushed, PushOutcome::Failed("x".into()));
    }

    #[test]
    fn test_repo_sync_status_variants() {
        assert_eq!(RepoSyncStatus::Clean, RepoSyncStatus::Clean);
        assert_eq!(RepoSyncStatus::Ahead, RepoSyncStatus::Ahead);
        assert_eq!(RepoSyncStatus::Behind, RepoSyncStatus::Behind);
        assert_eq!(RepoSyncStatus::Diverged, RepoSyncStatus::Diverged);
        assert_ne!(RepoSyncStatus::Clean, RepoSyncStatus::Ahead);
    }

    #[test]
    fn test_lib_git_client_default() {
        let client = LibGitClient;
        let _ = client;
    }

    /// Detached HEAD guard: `ensure_head_branch_name` rejects detached HEAD.
    #[test]
    fn test_ensure_head_branch_name_rejects_detached_head() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::now("tester", "t@test.com").unwrap();
        let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let commit_id = repo
            .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();
        repo.set_head_detached(commit_id).unwrap();
        let head = repo.head().unwrap();
        let result = LibGitClient::ensure_head_branch_name(&head);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("detached"));
        assert!(
            msg.contains("checkout a branch first"),
            "expected checkout guidance in: {msg}"
        );
    }

    /// Upstream fallback: when branch has no upstream, returns origin/<branch>.
    #[test]
    fn test_resolve_upstream_ref_fallback() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::now("tester", "t@test.com").unwrap();
        let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("refs/heads/feature"), &sig, &sig, "init", &tree, &[])
            .unwrap();
        repo.set_head("refs/heads/feature").unwrap();
        let upstream = LibGitClient::resolve_upstream_ref(&repo, "feature");
        assert_eq!(upstream, "refs/remotes/origin/feature");
    }

    /// Upstream preference: when upstream is configured, uses that ref.
    #[test]
    fn test_resolve_upstream_ref_prefers_upstream() {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        repo.remote("origin", "https://example.com/repo.git")
            .unwrap();
        let sig = Signature::now("tester", "t@test.com").unwrap();
        let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let oid = repo
            .commit(Some("refs/heads/main"), &sig, &sig, "init", &tree, &[])
            .unwrap();
        repo.reference("refs/remotes/origin/main", oid, true, "ref")
            .unwrap();
        repo.reference("refs/remotes/origin/tracked", oid, true, "ref")
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();
        let mut branch = repo.find_branch("main", BranchType::Local).unwrap();
        branch.set_upstream(Some("origin/tracked")).unwrap();
        let upstream = LibGitClient::resolve_upstream_ref(&repo, "main");
        assert_eq!(upstream, "refs/remotes/origin/tracked");
    }

    /// `pull()` rejects detached HEAD before fetching.
    #[test]
    fn test_pull_rejects_detached_head() {
        let dir = TempDir::new().unwrap();
        {
            let repo = Repository::init(dir.path()).unwrap();
            let sig = Signature::now("tester", "t@test.com").unwrap();
            let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let oid = repo
                .commit(Some("refs/heads/main"), &sig, &sig, "init", &tree, &[])
                .unwrap();
            repo.set_head_detached(oid).unwrap();
        }
        let client = LibGitClient::new();
        let result = client.pull(dir.path(), None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("detached"));
    }

    /// `pull()` must not overwrite a dirty worktree during fast-forward.
    #[test]
    fn test_pull_does_not_clobber_dirty_worktree() {
        let remote_dir = TempDir::new().unwrap();
        let _remote = Repository::init_bare(remote_dir.path()).unwrap();

        // Seed remote with one commit on default branch.
        let seed_dir = TempDir::new().unwrap();
        let seed = Repository::init(seed_dir.path()).unwrap();
        std::fs::write(seed_dir.path().join("file.txt"), "base\n").unwrap();
        let mut seed_index = seed.index().unwrap();
        seed_index
            .add_path(std::path::Path::new("file.txt"))
            .unwrap();
        seed_index.write().unwrap();
        let seed_tree_id = seed_index.write_tree().unwrap();
        let seed_tree = seed.find_tree(seed_tree_id).unwrap();
        let sig = Signature::now("tester", "t@test.com").unwrap();
        seed.commit(Some("HEAD"), &sig, &sig, "init", &seed_tree, &[])
            .unwrap();
        seed.remote("origin", remote_dir.path().to_str().unwrap())
            .unwrap();
        let seed_branch = seed
            .head()
            .unwrap()
            .shorthand()
            .unwrap_or("main")
            .to_string();
        let seed_branch_ref = format!("refs/heads/{seed_branch}");
        let mut seed_origin = seed.find_remote("origin").unwrap();
        seed_origin
            .push(&[format!("{seed_branch_ref}:{seed_branch_ref}")], None)
            .unwrap();

        // Local clone with dirty working tree.
        let local_dir = TempDir::new().unwrap();
        let _local =
            Repository::clone(remote_dir.path().to_str().unwrap(), local_dir.path()).unwrap();
        std::fs::write(local_dir.path().join("file.txt"), "local-dirty\n").unwrap();

        // Separate clone advances remote.
        let updater_dir = TempDir::new().unwrap();
        let updater =
            Repository::clone(remote_dir.path().to_str().unwrap(), updater_dir.path()).unwrap();
        std::fs::write(updater_dir.path().join("file.txt"), "remote-change\n").unwrap();
        let mut updater_index = updater.index().unwrap();
        updater_index
            .add_path(std::path::Path::new("file.txt"))
            .unwrap();
        updater_index.write().unwrap();
        let updater_tree_id = updater_index.write_tree().unwrap();
        let updater_tree = updater.find_tree(updater_tree_id).unwrap();
        let updater_parent = updater.head().unwrap().peel_to_commit().unwrap();
        let remote_oid = updater
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                "remote update",
                &updater_tree,
                &[&updater_parent],
            )
            .unwrap();
        let updater_branch = updater
            .head()
            .unwrap()
            .shorthand()
            .unwrap_or("main")
            .to_string();
        let updater_branch_ref = format!("refs/heads/{updater_branch}");
        let mut updater_origin = updater.find_remote("origin").unwrap();
        updater_origin
            .push(
                &[format!("{updater_branch_ref}:{updater_branch_ref}")],
                None,
            )
            .unwrap();

        let client = LibGitClient::new();
        let result = client.pull(local_dir.path(), None);
        assert!(result.is_ok());

        // Branch fast-forwards to remote tip.
        let local_repo = Repository::open(local_dir.path()).unwrap();
        let local_head = local_repo.head().unwrap().target().unwrap();
        assert_eq!(local_head, remote_oid);

        // Local uncommitted content remains untouched.
        let local_content = std::fs::read_to_string(local_dir.path().join("file.txt")).unwrap();
        assert_eq!(local_content, "local-dirty\n");
    }

    /// Initialize a repo with one commit on `refs/heads/main` and return its OID.
    fn init_repo_with_commit(dir: &Path) -> git2::Oid {
        let repo = Repository::init(dir).unwrap();
        let sig = Signature::now("tester", "t@test.com").unwrap();
        let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let oid = repo
            .commit(Some("refs/heads/main"), &sig, &sig, "init", &tree, &[])
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();
        oid
    }

    /// `status()` on a freshly initialised repo: no files, no upstream, clean.
    #[test]
    fn test_status_fresh_repo_is_clean() {
        let dir = TempDir::new().unwrap();
        let _repo = Repository::init(dir.path()).unwrap();

        let client = LibGitClient::new();
        let status = client.status(dir.path()).unwrap();

        assert!(
            status.files.is_empty(),
            "expected no files: {:?}",
            status.files
        );
        assert!(!status.dirty);
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
        assert!(!status.branch.is_empty(), "expected symbolic branch name");
    }

    /// `status()` flags an untracked file with `FileChange::Untracked`.
    #[test]
    fn test_status_reports_untracked_file() {
        let dir = TempDir::new().unwrap();
        init_repo_with_commit(dir.path());
        std::fs::write(dir.path().join("new.txt"), "hello\n").unwrap();

        let client = LibGitClient::new();
        let status = client.status(dir.path()).unwrap();

        assert!(status.dirty);
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
        assert_eq!(status.files.len(), 1);
        let file = status.files.first().expect("one file");
        assert_eq!(file.path, "new.txt");
        assert_eq!(file.status, FileChange::Untracked);
    }

    /// `status()` reports both staged-new (`Added`) and tracked-modified (`Modified`).
    #[test]
    fn test_status_reports_added_and_modified() {
        let dir = TempDir::new().unwrap();
        // Seed with a tracked file so we can modify it.
        let repo = Repository::init(dir.path()).unwrap();
        std::fs::write(dir.path().join("tracked.txt"), "v1\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("tracked.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = Signature::now("tester", "t@test.com").unwrap();
        repo.commit(Some("refs/heads/main"), &sig, &sig, "seed", &tree, &[])
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();

        // Modify tracked file in worktree (no staging).
        std::fs::write(dir.path().join("tracked.txt"), "v2\n").unwrap();

        // Stage a new file (Added in the index).
        std::fs::write(dir.path().join("staged.txt"), "new\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("staged.txt")).unwrap();
        index.write().unwrap();

        let client = LibGitClient::new();
        let status = client.status(dir.path()).unwrap();

        assert!(status.dirty);
        let by_path: std::collections::HashMap<&str, &FileChange> = status
            .files
            .iter()
            .map(|f| (f.path.as_str(), &f.status))
            .collect();
        assert_eq!(by_path.get("tracked.txt"), Some(&&FileChange::Modified));
        assert_eq!(by_path.get("staged.txt"), Some(&&FileChange::Added));
    }

    /// `checkout(create=true)` creates a fresh branch from HEAD and switches to it.
    #[test]
    fn test_checkout_create_fresh_branch_succeeds() {
        let dir = TempDir::new().unwrap();
        init_repo_with_commit(dir.path());

        let client = LibGitClient::new();
        client.checkout(dir.path(), "feature", true).unwrap();

        let repo = Repository::open(dir.path()).unwrap();
        let head = repo.head().unwrap();
        assert_eq!(head.shorthand(), Some("feature"));
    }

    /// `checkout(create=true)` for an existing branch name returns `AlreadyExists`.
    #[test]
    fn test_checkout_create_existing_branch_errors() {
        let dir = TempDir::new().unwrap();
        init_repo_with_commit(dir.path());

        let client = LibGitClient::new();
        client.checkout(dir.path(), "feature", true).unwrap();

        let err = client
            .checkout(dir.path(), "feature", true)
            .expect_err("expected AlreadyExists");
        assert!(
            matches!(
                err,
                AppError::AlreadyExists { ref entity, ref id }
                    if entity == "branch" && id == "feature"
            ),
            "expected AlreadyExists, got: {err:?}"
        );
    }

    /// `checkout(create=false)` switches to an existing branch.
    #[test]
    fn test_checkout_no_create_existing_branch_succeeds() {
        let dir = TempDir::new().unwrap();
        init_repo_with_commit(dir.path());

        let client = LibGitClient::new();
        client.checkout(dir.path(), "feature", true).unwrap();
        client.checkout(dir.path(), "main", false).unwrap();

        let repo = Repository::open(dir.path()).unwrap();
        assert_eq!(repo.head().unwrap().shorthand(), Some("main"));
    }

    /// `checkout(create=false)` for a missing branch surfaces a git error.
    #[test]
    fn test_checkout_no_create_missing_branch_errors() {
        let dir = TempDir::new().unwrap();
        init_repo_with_commit(dir.path());

        let client = LibGitClient::new();
        let err = client
            .checkout(dir.path(), "does-not-exist", false)
            .expect_err("expected error");
        assert!(matches!(err, AppError::GitFailed { .. }), "got: {err:?}");
    }

    /// `add_remote` registers a new remote; a second add with the same name fails.
    #[test]
    fn test_add_remote_succeeds_and_rejects_duplicates() {
        let dir = TempDir::new().unwrap();
        let _repo = Repository::init(dir.path()).unwrap();

        let client = LibGitClient::new();
        client
            .add_remote(dir.path(), "origin", "https://example.com/repo.git")
            .unwrap();

        let repo = Repository::open(dir.path()).unwrap();
        let remote = repo.find_remote("origin").unwrap();
        assert_eq!(remote.url(), Some("https://example.com/repo.git"));

        let err = client
            .add_remote(dir.path(), "origin", "https://example.com/other.git")
            .expect_err("expected duplicate remote to error");
        assert!(matches!(err, AppError::GitFailed { .. }), "got: {err:?}");
    }

    /// `has_remote` returns `false` for missing remotes and `true` after `add_remote`.
    #[test]
    fn test_has_remote_reflects_add_remote() {
        let dir = TempDir::new().unwrap();
        let _repo = Repository::init(dir.path()).unwrap();
        let client = LibGitClient::new();

        assert!(!client.has_remote(dir.path(), "origin").unwrap());

        client
            .add_remote(dir.path(), "origin", "https://example.com/repo.git")
            .unwrap();
        assert!(client.has_remote(dir.path(), "origin").unwrap());
        assert!(!client.has_remote(dir.path(), "upstream").unwrap());
    }

    /// `remote_url` returns the configured URL for a known remote.
    #[test]
    fn test_remote_url_returns_configured_url() {
        let dir = TempDir::new().unwrap();
        let _repo = Repository::init(dir.path()).unwrap();
        let client = LibGitClient::new();

        client
            .add_remote(dir.path(), "origin", "https://example.com/owner/repo.git")
            .unwrap();
        let url = client.remote_url(dir.path(), "origin").unwrap();
        assert_eq!(url.as_deref(), Some("https://example.com/owner/repo.git"));
    }

    /// `remote_url` errors when the remote does not exist.
    #[test]
    fn test_remote_url_errors_on_missing_remote() {
        let dir = TempDir::new().unwrap();
        let _repo = Repository::init(dir.path()).unwrap();
        let client = LibGitClient::new();

        let err = client
            .remote_url(dir.path(), "missing")
            .expect_err("expected error for missing remote");
        assert!(matches!(err, AppError::GitFailed { .. }), "got: {err:?}");
    }

    /// `current_commit_sha` returns the 40-char hex of HEAD's commit.
    #[test]
    fn test_current_commit_sha_matches_head() {
        let dir = TempDir::new().unwrap();
        let oid = init_repo_with_commit(dir.path());

        let client = LibGitClient::new();
        let sha = client.current_commit_sha(dir.path()).unwrap();

        assert_eq!(sha.len(), 40, "expected 40-char hex sha, got: {sha}");
        assert_eq!(sha, oid.to_string());
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// `current_branch_label` synthesizes an informational name in detached HEAD.
    #[test]
    fn test_current_branch_label_handles_detached_head() {
        let dir = TempDir::new().unwrap();
        let oid = init_repo_with_commit(dir.path());
        let repo = Repository::open(dir.path()).unwrap();
        repo.set_head_detached(oid).unwrap();

        let label = current_branch_label(&repo).unwrap();
        assert!(label.starts_with("(HEAD detached at "), "got: {label}");
        assert!(label.contains(&oid.to_string()[..7]));
    }

    /// `FileChange::as_str` matches the serde camelCase wire shape, so
    /// MCP callers can use either pathway without drift.
    #[test]
    fn test_file_change_as_str_matches_serde() {
        for change in [
            FileChange::Added,
            FileChange::Modified,
            FileChange::Deleted,
            FileChange::Untracked,
            FileChange::Renamed,
        ] {
            let json = serde_json::to_string(&change).unwrap();
            // Strip the JSON quotes for comparison: `"added"` -> `added`.
            let trimmed = json.trim_matches('"');
            assert_eq!(trimmed, change.as_str(), "drift for {change:?}");
        }
    }

    /// `classify_file_status` applies the documented priority order.
    #[test]
    fn test_classify_file_status_priority() {
        use git2::Status;
        assert_eq!(
            classify_file_status(Status::INDEX_RENAMED | Status::WT_MODIFIED),
            Some(FileChange::Renamed)
        );
        assert_eq!(
            classify_file_status(Status::WT_DELETED | Status::INDEX_NEW),
            Some(FileChange::Deleted)
        );
        assert_eq!(
            classify_file_status(Status::INDEX_NEW | Status::WT_MODIFIED),
            Some(FileChange::Added)
        );
        assert_eq!(
            classify_file_status(Status::WT_MODIFIED),
            Some(FileChange::Modified)
        );
        assert_eq!(
            classify_file_status(Status::WT_NEW),
            Some(FileChange::Untracked)
        );
        assert_eq!(classify_file_status(Status::CURRENT), None);
    }
}
