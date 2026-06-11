//! Host-side path validation for MCP file tools.
//!
//! Mirrors agent/src/service.rs:105-150 but operates on host paths under
//! the project's local checkout (any absolute path under `Project::repo_path()`).
//! Pre-validate the requested path, canonicalize, post-check the canonical
//! result stays within the project root. Symlink escapes are caught by the
//! post-check because `canonicalize` follows symlinks.
//!
//! Caveat: this validator resolves the path once and returns the canonical
//! result. It is NOT a defense against TOCTOU swaps between resolution and
//! the caller's subsequent `open()`. The v1.1 threat model assumes the agent
//! is trusted within the project tree. If you ever reuse this for an
//! untrusted-agent scenario, additionally open the canonical path with
//! `O_NOFOLLOW` or use `openat()`-by-fd to defeat symlink races.

use std::path::{Component, Path, PathBuf};

use crate::error::AppError;

pub fn safe_join_within_repo(
    repo_root: &Path,
    requested: &str,
    must_exist: bool,
) -> Result<PathBuf, AppError> {
    if requested.is_empty() {
        return Err(invalid_path("path is empty"));
    }
    let joined = repo_root.join(requested);
    if joined
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err(invalid_path("path contains '..' traversal"));
    }
    let canonical_root = std::fs::canonicalize(repo_root).map_err(|e| AppError::Internal {
        reason: format!("cannot canonicalize repo root {}: {e}", repo_root.display()),
    })?;

    if must_exist {
        let canonical = std::fs::canonicalize(&joined).map_err(|e| AppError::InvalidInput {
            field: "path".into(),
            reason: format!("cannot resolve {}: {e}", joined.display()),
        })?;
        if !canonical.starts_with(&canonical_root) {
            return Err(invalid_path("path escapes project root"));
        }
        Ok(canonical)
    } else {
        let parent = joined
            .parent()
            .ok_or_else(|| invalid_path("path has no parent"))?;
        // Pre-check at the string level: catches absolute paths outside the repo
        // (e.g. requested = "/tmp/x") BEFORE we mutate the filesystem with mkdir.
        // Without this guard, create_dir_all would create attacker-controlled
        // directories anywhere on the host before the post-check rejects.
        if !parent.starts_with(repo_root) {
            return Err(invalid_path("path escapes project root"));
        }
        std::fs::create_dir_all(parent).map_err(|e| AppError::Internal {
            reason: format!("cannot create parent {}: {e}", parent.display()),
        })?;
        let canonical_parent =
            std::fs::canonicalize(parent).map_err(|e| AppError::InvalidInput {
                field: "path".into(),
                reason: format!("cannot resolve parent of {}: {e}", joined.display()),
            })?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(invalid_path("path escapes project root"));
        }
        let file_name = joined
            .file_name()
            .ok_or_else(|| invalid_path("path has no file name"))?;
        Ok(canonical_parent.join(file_name))
    }
}

fn invalid_path(reason: &str) -> AppError {
    AppError::InvalidInput {
        field: "path".into(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("hello.txt"), b"world").unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/nested.txt"), b"nest").unwrap();
        dir
    }

    #[test]
    fn safe_join_resolves_existing_file() {
        let repo = setup_repo();
        let resolved = safe_join_within_repo(repo.path(), "hello.txt", true).unwrap();
        assert!(
            resolved.ends_with("hello.txt"),
            "expected to end with hello.txt: {}",
            resolved.display()
        );
    }

    #[test]
    fn safe_join_resolves_nested_existing_file() {
        let repo = setup_repo();
        let resolved = safe_join_within_repo(repo.path(), "sub/nested.txt", true).unwrap();
        assert!(
            resolved.ends_with("sub/nested.txt"),
            "expected to end with sub/nested.txt: {}",
            resolved.display()
        );
    }

    #[test]
    fn safe_join_rejects_parent_traversal() {
        let repo = setup_repo();
        let err = safe_join_within_repo(repo.path(), "../outside", true).unwrap_err();
        let AppError::InvalidInput { reason, .. } = err else {
            unreachable!("expected InvalidInput, got {err:?}")
        };
        assert!(
            reason.contains(".."),
            "expected reason to mention '..': {reason}"
        );
    }

    #[test]
    fn safe_join_rejects_absolute_path_outside_repo() {
        let repo = setup_repo();
        let err = safe_join_within_repo(repo.path(), "/etc/passwd", true).unwrap_err();
        assert!(
            matches!(err, AppError::InvalidInput { .. }),
            "expected InvalidInput, got {err:?}"
        );
    }

    #[test]
    fn safe_join_rejects_symlink_escape() {
        let repo = setup_repo();
        let outside = tempfile::tempdir().expect("outside tempdir");
        std::fs::write(outside.path().join("secret.txt"), b"top secret").unwrap();
        let link = repo.path().join("evil-link");
        std::os::unix::fs::symlink(outside.path().join("secret.txt"), &link).unwrap();
        let err = safe_join_within_repo(repo.path(), "evil-link", true).unwrap_err();
        let AppError::InvalidInput { reason, .. } = err else {
            unreachable!("expected InvalidInput, got {err:?}")
        };
        assert!(
            reason.contains("escape"),
            "expected reason to mention 'escape': {reason}"
        );
    }

    #[test]
    fn safe_join_for_write_allows_nonexistent_leaf() {
        let repo = setup_repo();
        let resolved = safe_join_within_repo(repo.path(), "new-file.txt", false).unwrap();
        assert!(
            resolved.ends_with("new-file.txt"),
            "expected to end with new-file.txt: {}",
            resolved.display()
        );
        assert!(
            !resolved.exists(),
            "did not expect file to exist yet: {}",
            resolved.display()
        );
    }

    #[test]
    fn safe_join_for_write_creates_parent() {
        let repo = setup_repo();
        let resolved =
            safe_join_within_repo(repo.path(), "deep/nested/path/file.txt", false).unwrap();
        let parent = resolved.parent().expect("resolved must have a parent");
        assert!(
            parent.exists(),
            "expected parent to be created: {}",
            parent.display()
        );
        assert!(
            !resolved.exists(),
            "did not expect leaf file to exist yet: {}",
            resolved.display()
        );
    }

    #[test]
    fn safe_join_for_write_does_not_create_dirs_outside_repo() {
        let repo = setup_repo();
        let outside = tempfile::tempdir().expect("outside tempdir");
        let leaked = outside.path().join("nonexistent-subdir");
        // The requested path is the absolute path of `leaked/file.txt` — outside
        // the repo. The validator must reject WITHOUT creating leaked/.
        let leaked_str = leaked.join("file.txt").to_string_lossy().into_owned();
        let result = safe_join_within_repo(repo.path(), &leaked_str, false);
        assert!(
            result.is_err(),
            "validator must reject absolute path outside repo"
        );
        assert!(
            !leaked.exists(),
            "validator must NOT create directories outside the repo"
        );
    }

    #[test]
    fn safe_join_rejects_empty_path() {
        let repo = setup_repo();
        let err = safe_join_within_repo(repo.path(), "", true).unwrap_err();
        assert!(
            matches!(err, AppError::InvalidInput { .. }),
            "expected InvalidInput, got {err:?}"
        );
    }
}
