//! Read/write the committed `.opnble` marker file.
//!
//! The marker is a small JSON file at the root of every imported project's
//! working tree. It survives `git clone` to other machines so MCP tools can
//! resolve a project's portable slug without machine-local state. It is
//! committed to git (NOT in `.gitignore`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Filename written at the repo root.
const FILE_NAME: &str = ".opnble";

/// Current schema version. Bumped only when an existing field changes shape;
/// adding a new optional field does not require a bump.
const SCHEMA_VERSION: u32 = 1;

/// On-disk shape of `.opnble`.
///
/// `deny_unknown_fields` makes future schema drift surface as a clean error
/// instead of silently dropping fields the writer cared about. The version
/// field lets older builds notice they are reading a newer marker and refuse
/// rather than misinterpret it.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    pub version: u32,
    pub slug: String,
    pub repo: String,
}

impl Marker {
    /// Build a marker at the current schema version.
    pub fn new(slug: String, repo: String) -> Self {
        Self {
            version: SCHEMA_VERSION,
            slug,
            repo,
        }
    }
}

/// Resolve the marker path for the given repo root.
fn marker_path(repo_path: &Path) -> PathBuf {
    repo_path.join(FILE_NAME)
}

/// Write `<repo_path>/.opnble`. Idempotent (overwrites). Pretty JSON, two
/// space indent, trailing newline so the committed file diffs cleanly.
pub fn write_marker(repo_path: &Path, marker: &Marker) -> Result<(), AppError> {
    let path = marker_path(repo_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut body = serde_json::to_string_pretty(marker)?;
    body.push('\n');
    std::fs::write(&path, body)?;
    Ok(())
}

/// Read `<repo_path>/.opnble`. Returns `Ok(None)` when the file is absent;
/// returns `Err` when the file exists but does not parse cleanly (invalid
/// JSON, missing field, unknown field).
pub fn read_marker(repo_path: &Path) -> Result<Option<Marker>, AppError> {
    let path = marker_path(repo_path);
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let marker: Marker = serde_json::from_str(&raw).map_err(|e| AppError::StorageFailed {
        reason: format!("cannot parse {}: {e}", path.display()),
    })?;
    Ok(Some(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn write_then_read_round_trip() {
        let dir = TempDir::new().expect("tempdir");
        let marker = Marker::new(
            "my-blog".to_string(),
            "https://github.com/owner/my-blog.git".to_string(),
        );
        write_marker(dir.path(), &marker).expect("write");
        let loaded = read_marker(dir.path()).expect("read").expect("present");
        assert_eq!(loaded, marker);
    }

    #[test]
    fn write_emits_pretty_json_with_trailing_newline() {
        let dir = TempDir::new().expect("tempdir");
        let marker = Marker::new(
            "my-blog".to_string(),
            "https://github.com/owner/my-blog.git".to_string(),
        );
        write_marker(dir.path(), &marker).expect("write");
        let body = std::fs::read_to_string(dir.path().join(".opnble")).expect("read");
        assert!(body.ends_with('\n'), "marker should end with a newline");
        assert!(body.contains("\n  \"slug\""), "expected two-space indent");
    }

    #[test]
    fn write_overwrites_existing_marker() {
        let dir = TempDir::new().expect("tempdir");
        let first = Marker::new("old".to_string(), "https://a/old.git".to_string());
        write_marker(dir.path(), &first).expect("write first");
        let second = Marker::new("new".to_string(), "https://a/new.git".to_string());
        write_marker(dir.path(), &second).expect("write second");
        let loaded = read_marker(dir.path()).expect("read").expect("present");
        assert_eq!(loaded.slug, "new");
    }

    #[test]
    fn read_returns_none_when_missing() {
        let dir = TempDir::new().expect("tempdir");
        let outcome = read_marker(dir.path()).expect("ok");
        assert!(outcome.is_none());
    }

    #[test]
    fn read_returns_err_when_invalid_json() {
        let dir = TempDir::new().expect("tempdir");
        std::fs::write(dir.path().join(".opnble"), b"{ broken").expect("write");
        let err = read_marker(dir.path()).expect_err("invalid json should error");
        assert!(matches!(err, AppError::StorageFailed { .. }));
    }

    #[test]
    fn read_rejects_unknown_fields() {
        let dir = TempDir::new().expect("tempdir");
        let body = r#"{
            "version": 1,
            "slug": "blog",
            "repo": "https://a/blog.git",
            "futureField": "surprise"
        }"#;
        std::fs::write(dir.path().join(".opnble"), body).expect("write");
        let err = read_marker(dir.path()).expect_err("unknown field should error");
        assert!(matches!(err, AppError::StorageFailed { .. }));
    }

    #[test]
    fn read_rejects_missing_required_field() {
        let dir = TempDir::new().expect("tempdir");
        let body = r#"{ "version": 1, "slug": "blog" }"#;
        std::fs::write(dir.path().join(".opnble"), body).expect("write");
        let err = read_marker(dir.path()).expect_err("missing repo should error");
        assert!(matches!(err, AppError::StorageFailed { .. }));
    }
}
