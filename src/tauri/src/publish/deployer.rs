//! Publish orchestrator
//!
//! Coordinates framework detection, container build, gh-pages push,
//! and GitHub Pages API enablement.

use std::path::Path;

use crate::auth::Authenticator;
use crate::error::AppError;
use crate::infrastructure::git::{GitOps, PushOutcome};
use crate::infrastructure::runtime::Runtime;
use crate::projects::store::ProjectStore;
use crate::projects::types::ProjectId;
use crate::publish::builder;
use crate::publish::github_pages::GitHubPagesClient;
use crate::publish::types::{Framework, RepoIdentity};
use crate::publish::PublishError;

const GH_PAGES_BRANCH: &str = "gh-pages";
const COMMIT_MESSAGE: &str = "deploy to GitHub Pages via Opnble";

/// Dependencies for the publish operation.
pub struct PublishDeps<'deps> {
    pub store: &'deps dyn ProjectStore,
    pub runtime: &'deps dyn Runtime,
    pub git: &'deps dyn GitOps,
    pub authenticator: &'deps Authenticator,
    pub pages_client: &'deps GitHubPagesClient,
}

/// Dependencies for the unpublish operation (no runtime or git needed).
pub struct UnpublishDeps<'deps> {
    pub store: &'deps dyn ProjectStore,
    pub authenticator: &'deps Authenticator,
    pub pages_client: &'deps GitHubPagesClient,
}

/// Parse owner/repo from a GitHub URL.
///
/// Supports HTTPS and SSH formats:
/// - `https://github.com/owner/repo.git`
/// - `https://github.com/owner/repo`
/// - `git@github.com:owner/repo.git`
/// - `git@github.com:owner/repo`
pub fn parse_github_identity(repo_url: &str) -> Result<RepoIdentity, AppError> {
    let trimmed = repo_url.trim();

    let path_part = if let Some(rest) = trimmed.strip_prefix("git@github.com:") {
        rest.to_string()
    } else if let Some(rest) = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
    {
        rest.to_string()
    } else {
        return Err(invalid_repo_url(trimmed));
    };

    let path_part = path_part.trim_end_matches(".git");
    let parts: Vec<&str> = path_part.split('/').collect();

    let owner = parts.first().copied().filter(|s| !s.is_empty());
    let repo = parts.get(1).copied().filter(|s| !s.is_empty());

    match (owner, repo) {
        (Some(o), Some(r)) => Ok(RepoIdentity {
            owner: o.to_string(),
            repo: r.to_string(),
        }),
        _ => Err(invalid_repo_url(trimmed)),
    }
}

fn invalid_repo_url(url: &str) -> AppError {
    AppError::InvalidInput {
        field: "repo_url".into(),
        reason: format!("cannot parse GitHub owner/repo from URL: {url}"),
    }
}

/// Full publish flow: detect framework, build, push to gh-pages, enable Pages.
///
/// Returns both the live Pages URL and the detected `Framework` so callers
/// (Tauri command, MCP `github_pages_publish`) can surface the framework
/// without re-running detection on the worktree.
pub async fn publish(
    project_id: &ProjectId,
    deps: &PublishDeps<'_>,
) -> Result<(String, Framework), AppError> {
    let project = deps.store.get(project_id)?;
    let repo_path = project.repo_path();

    // 1. Validate URL shape (cheap, no I/O)
    let identity = parse_github_identity(&project.repo_url)?;

    // 2. Detect framework and build config (local filesystem only)
    let mut build_config = builder::detect_framework(&repo_path)?;
    builder::with_base_path(&mut build_config, &identity.repo);
    let framework = build_config.framework.clone();

    // 3. Resolve GitHub token (keychain access)
    let token = deps
        .authenticator
        .resolve_push_token(&project.repo_url)
        .await?
        .ok_or_else(|| AppError::AuthRequired {
            provider: "github".into(),
        })?;

    // 4. Check repo visibility (private repos need Pro plan)
    if deps.pages_client.is_repo_private(&identity, &token).await? {
        return Err(PublishError::PrivateRepoNeedsPro.into());
    }

    // 5. Run build in container
    builder::run_build(deps.runtime, project_id.as_str(), &repo_path, &build_config).await?;

    // 6. Resolve output directory (probe alternatives for Unknown framework)
    let output_dir = resolve_output_dir(&repo_path, &build_config)?;

    // 7. Push to gh-pages
    let push_result = deps.git.push_orphan_branch(
        &repo_path,
        GH_PAGES_BRANCH,
        &output_dir,
        COMMIT_MESSAGE,
        Some(&token),
    )?;

    match push_result {
        PushOutcome::Pushed => {}
        PushOutcome::NothingToPush => {
            // Shouldn't happen with force push, but handle gracefully
            tracing::warn!("gh-pages push returned NothingToPush");
        }
        PushOutcome::Failed(reason) => {
            return Err(PublishError::PushFailed { reason }.into());
        }
    }

    // 8. Enable GitHub Pages via API
    let pages_url = deps.pages_client.enable_pages(&identity, &token).await?;

    // 9. Wait for GitHub Pages deployment to complete
    let pages_url = deps
        .pages_client
        .wait_for_deployment(&identity, &token, &pages_url)
        .await?;

    // 10. Persist pages_url on the project
    let mut updated = deps.store.get(project_id)?;
    updated.pages_url = Some(pages_url.clone());
    deps.store.update(updated)?;

    Ok((pages_url, framework))
}

/// Unpublish: disable Pages, delete gh-pages branch, clear `pages_url`.
pub async fn unpublish(project_id: &ProjectId, deps: &UnpublishDeps<'_>) -> Result<(), AppError> {
    let project = deps.store.get(project_id)?;

    let token = deps
        .authenticator
        .resolve_push_token(&project.repo_url)
        .await?
        .ok_or_else(|| AppError::AuthRequired {
            provider: "github".into(),
        })?;

    let identity = parse_github_identity(&project.repo_url)?;

    // Disable Pages (best-effort: cleanup should not block unpublish)
    if let Err(e) = deps.pages_client.disable_pages(&identity, &token).await {
        tracing::warn!(
            "cannot disable GitHub Pages for {}/{}: {e}",
            identity.owner,
            identity.repo
        );
    }

    // Delete gh-pages branch on remote (best-effort: branch may already be gone)
    if let Err(e) = delete_remote_branch(&project.repo_path(), GH_PAGES_BRANCH, &token) {
        tracing::warn!("cannot delete remote branch {GH_PAGES_BRANCH}: {e}");
    }

    // Clear pages_url
    let mut updated = deps.store.get(project_id)?;
    updated.pages_url = None;
    deps.store.update(updated)?;

    Ok(())
}

/// Resolve the actual output directory, probing alternatives for Unknown framework.
fn resolve_output_dir(
    repo_path: &Path,
    config: &crate::publish::types::BuildConfig,
) -> Result<std::path::PathBuf, AppError> {
    let primary = repo_path.join(&config.output_dir);
    if primary.is_dir() {
        return Ok(primary);
    }

    // For Unknown framework, try alternatives
    if config.framework == Framework::Unknown {
        for alt in &["dist", "build", "out", "public"] {
            let alt_path = repo_path.join(alt);
            if alt_path.is_dir() {
                return Ok(alt_path);
            }
        }
    }

    Err(PublishError::BuildOutputMissing {
        expected: config.output_dir.clone(),
    }
    .into())
}

/// Delete a branch on the remote by pushing an empty refspec.
fn delete_remote_branch(repo_path: &Path, branch: &str, token: &str) -> Result<(), AppError> {
    let repo = git2::Repository::open(repo_path)?;
    let refspec = format!(":refs/heads/{branch}");
    let mut remote = repo.find_remote("origin")?;

    let mut push_opts = git2::PushOptions::new();
    let mut callbacks = git2::RemoteCallbacks::new();
    let token = token.to_owned();
    callbacks.credentials(move |_url, _username, _allowed| {
        git2::Cred::userpass_plaintext("oauth2", &token)
    });
    push_opts.remote_callbacks(callbacks);

    remote
        .push(&[refspec.as_str()], Some(&mut push_opts))
        .map_err(|e| {
            AppError::Publish(PublishError::PushFailed {
                reason: format!("cannot delete remote branch {branch}: {e}"),
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_https_url() {
        let id =
            parse_github_identity("https://github.com/owner/repo.git").expect("should parse https");
        assert_eq!(id.owner, "owner");
        assert_eq!(id.repo, "repo");
    }

    #[test]
    fn parse_https_url_no_git_suffix() {
        let id =
            parse_github_identity("https://github.com/owner/repo").expect("should parse https");
        assert_eq!(id.owner, "owner");
        assert_eq!(id.repo, "repo");
    }

    #[test]
    fn parse_ssh_url() {
        let id = parse_github_identity("git@github.com:owner/repo.git").expect("should parse ssh");
        assert_eq!(id.owner, "owner");
        assert_eq!(id.repo, "repo");
    }

    #[test]
    fn parse_ssh_url_no_git_suffix() {
        let id = parse_github_identity("git@github.com:owner/repo").expect("should parse ssh");
        assert_eq!(id.owner, "owner");
        assert_eq!(id.repo, "repo");
    }

    #[test]
    fn parse_non_github_url_fails() {
        let result = parse_github_identity("https://gitlab.com/owner/repo");
        assert!(result.is_err());
    }

    #[test]
    fn parse_empty_owner_fails() {
        let result = parse_github_identity("https://github.com//repo");
        assert!(result.is_err());
    }

    #[test]
    fn parse_missing_repo_fails() {
        let result = parse_github_identity("https://github.com/owner");
        assert!(result.is_err());
    }

    #[test]
    fn parse_with_whitespace() {
        let id = parse_github_identity("  https://github.com/owner/repo.git  ")
            .expect("should handle whitespace");
        assert_eq!(id.owner, "owner");
        assert_eq!(id.repo, "repo");
    }
}
