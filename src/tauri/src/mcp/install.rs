//! Install Opnble's MCP entry into every place Cursor reads `mcp.json`:
//!
//! - `<repo>/.cursor/mcp.json` for each imported project (written at clone
//!   time, gitignored because the bearer token lives in it).
//! - `~/.cursor/mcp.json` for global Cursor use (written at bootstrap so a
//!   user who opens *any* folder in Cursor still sees `opnble.*` tools
//!   without having to import that folder through Opnble first).
//!
//! Both paths use the same deep-merge + atomic-write helper so token
//! rotation can sweep them uniformly. The `.opnble` marker that survives
//! `git clone` is written separately by [`crate::projects::marker`].

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::error::AppError;
use crate::mcp::{auth, config};
use crate::projects::store::ProjectStore;

/// Path written under each repo root, also the `.gitignore` entry.
///
/// One constant powers both because the file is what the line excludes;
/// keeping them in sync prevents `.cursor/mcp.json` from drifting from the
/// `.gitignore` line callers append.
pub const MCP_CONFIG_REL: &str = ".cursor/mcp.json";
/// Directory Cursor reads global MCP config from, resolved against
/// [`dirs::home_dir`]. Its presence on disk is also what we use to decide
/// whether Cursor is installed at all: when the directory is missing the
/// global install is a no-op so a user running Opnble without Cursor never
/// gets a stray `~/.cursor/` created out from under them.
const CURSOR_HOME_DIR: &str = ".cursor";
/// JSON key under which Cursor (and most other MCP-aware editors) read the
/// server bag.
const MCP_SERVERS_KEY: &str = "mcpServers";
/// Server name used inside `mcpServers`. Stable so token rotation can find
/// and update the entry without disturbing user-added entries.
const SERVER_NAME: &str = "opnble";

/// Aggregate result of [`sweep_token`] across every imported project.
#[derive(Debug, Clone, Default)]
pub struct SweepReport {
    /// Total imported projects considered (after filtering legacy / deleted).
    pub total: usize,
    /// Projects whose `.cursor/mcp.json` was rewritten with the new token.
    pub updated: usize,
    /// `(slug, error message)` for each project that failed to update.
    pub failed: Vec<(String, String)>,
}

/// Outcome of [`write_global_mcp_config`]: either the file was written (and
/// to where) or Cursor was not detected so the call was a no-op. Lets the
/// caller log the path on first install without growing a parallel result
/// channel.
#[expect(
    dead_code,
    reason = "dotapps no-ops the global cursor MCP install (bootstrap::install_global_cursor_mcp); kept for the upstream opnble flow"
)]
#[derive(Debug, Clone)]
pub enum GlobalInstall {
    /// `~/.cursor/` is missing, so Cursor is not installed on this machine
    /// and writing the file would create the directory out from under the
    /// user. Skipped silently.
    CursorNotDetected,
    /// `~/.cursor/mcp.json` was deep-merged with the `opnble` entry.
    Written { path: PathBuf },
}

/// Resolve `<repo>/.cursor/mcp.json`.
fn config_path(repo_path: &Path) -> PathBuf {
    repo_path.join(MCP_CONFIG_REL)
}

/// Resolve `~/.cursor/mcp.json` when Cursor is detected on disk.
///
/// Returns `Ok(None)` when `~/.cursor/` does not exist so a user who has
/// never installed Cursor never gets a stray config directory. Returns
/// `Err` only when the home directory itself cannot be resolved, which is
/// effectively unrecoverable on any supported platform.
fn global_config_path() -> Result<Option<PathBuf>, AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::StorageFailed {
        reason: "cannot resolve home directory for cursor global mcp.json".to_string(),
    })?;
    Ok(global_config_path_in(&home))
}

/// Path-resolution helper split out from [`global_config_path`] so tests
/// can drive the "Cursor present / absent" branches against a tempdir
/// without mutating `$HOME` (which clippy.toml bans for thread-safety).
fn global_config_path_in(home: &Path) -> Option<PathBuf> {
    let cursor_dir = home.join(CURSOR_HOME_DIR);
    if cursor_dir.exists() {
        Some(cursor_dir.join("mcp.json"))
    } else {
        None
    }
}

/// Deep-merge the `mcpServers.opnble` entry into `path`, preserving any
/// other servers and top-level keys the user has configured. Creates the
/// parent directory and the file if missing. If the file exists but does
/// NOT parse as a JSON object, returns Err so the UI can prompt the user
/// instead of clobbering their edits.
///
/// Behaviour is identical for project-scoped and global paths: only the
/// resolution differs, so callers (`write_project_mcp_config`,
/// `write_global_mcp_config`) are thin path-pickers over this helper.
pub fn write_mcp_config_at(path: &Path, token: &str, port: u16) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut root = read_existing_config(path)?;
    set_opnble_entry(&mut root, token, port)?;
    write_pretty(path, &root)
}

/// Write `<repo>/.cursor/mcp.json` so Cursor's project-scoped MCP picks up
/// the running Opnble server with the current bearer token.
pub fn write_project_mcp_config(repo_path: &Path, token: &str, port: u16) -> Result<(), AppError> {
    write_mcp_config_at(&config_path(repo_path), token, port)
}

/// Best-effort install of the project-scoped `.cursor/mcp.json` (current
/// bearer token + the fixed MCP port) and its `.gitignore` line.
///
/// Logs and continues on every failure so a caller's primary flow (import,
/// open-in-Cursor) is never blocked by a missing token or a read-only
/// working tree. `label` only scopes the log lines. Reuse this anywhere a
/// repo needs its Cursor MCP entry so legacy projects self-heal without each
/// call site re-deriving the token-then-write sequence.
pub fn install_project_entry(repo_path: &Path, label: &str) {
    let token = match auth::current_token() {
        Ok(token) if !token.is_empty() => token,
        Ok(_) => {
            tracing::warn!("skipping .cursor/mcp.json for {label}: empty mcp token");
            return;
        }
        Err(e) => {
            tracing::warn!("cannot read mcp token for {label}: {e}");
            return;
        }
    };
    if let Err(e) = write_project_mcp_config(repo_path, &token, config::PORT) {
        tracing::warn!("cannot install .cursor/mcp.json for {label}: {e}");
    }
    if let Err(e) = append_to_gitignore(repo_path, MCP_CONFIG_REL) {
        tracing::warn!("cannot patch .gitignore for {label}: {e}");
    }
}

/// Write `~/.cursor/mcp.json` so Cursor sees the `opnble.*` tool surface in
/// any window, not just folders Opnble imported. Skipped (returns
/// [`GlobalInstall::CursorNotDetected`]) when `~/.cursor/` does not exist.
///
/// This is the "no extra action" half of the install: per-project config
/// only helps when the user opens a folder Opnble cloned; the global file
/// also helps when they open something else in Cursor (e.g. an existing
/// project they cloned by hand). Both files are deep-merged so user-added
/// `mcpServers.*` entries always survive.
#[expect(
    dead_code,
    reason = "dotapps no-ops the global cursor MCP install (bootstrap::install_global_cursor_mcp); kept for the upstream opnble flow"
)]
pub fn write_global_mcp_config(token: &str, port: u16) -> Result<GlobalInstall, AppError> {
    match global_config_path()? {
        None => Ok(GlobalInstall::CursorNotDetected),
        Some(path) => {
            write_mcp_config_at(&path, token, port)?;
            Ok(GlobalInstall::Written { path })
        }
    }
}

/// Append a single line to `<repo>/.gitignore` if it is not already present
/// as a non-comment line. Idempotent. Creates the file if absent.
pub fn append_to_gitignore(repo_path: &Path, line: &str) -> Result<(), AppError> {
    let path = repo_path.join(".gitignore");
    let existing = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };

    if gitignore_contains(&existing, line) {
        return Ok(());
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(line);
    updated.push('\n');
    std::fs::write(&path, updated)?;
    Ok(())
}

/// On token rotation, sweep every imported project in the store and rewrite
/// its `.cursor/mcp.json` with the new token. Also rewrites
/// `~/.cursor/mcp.json` when present so the global install keeps working
/// without the user reconnecting Cursor. Best-effort: per-project (and
/// per-global-file) failures are logged via `tracing::warn!` and the
/// sweep continues.
///
/// Skips projects that have no slug (the importer attaches one but legacy
/// records may still be `None`) or whose local path no longer exists on
/// disk (deleted outside Opnble).
pub fn sweep_token(
    store: &dyn ProjectStore,
    new_token: &str,
    port: u16,
) -> Result<SweepReport, AppError> {
    let global = global_config_path()?;
    sweep_token_with_global(store, new_token, port, global.as_deref())
}

/// Path-injected variant of [`sweep_token`]: callers (tests) supply an
/// explicit `global` to avoid touching the developer's real
/// `~/.cursor/mcp.json` during `cargo test`. Pass `None` to skip the
/// global rewrite entirely.
fn sweep_token_with_global(
    store: &dyn ProjectStore,
    new_token: &str,
    port: u16,
    global: Option<&Path>,
) -> Result<SweepReport, AppError> {
    let projects = store.list()?;
    let mut report = SweepReport::default();

    if let Some(path) = global {
        match write_mcp_config_at(path, new_token, port) {
            Ok(()) => tracing::debug!(
                path = %path.display(),
                "rotated bearer token in global cursor mcp.json"
            ),
            Err(e) => tracing::warn!(
                path = %path.display(),
                error = %e,
                "cannot rewrite ~/.cursor/mcp.json during token sweep; \
                 global Cursor windows will need a manual reconnect"
            ),
        }
    }

    for project in projects {
        let Some(slug) = project.slug.as_deref() else {
            continue;
        };
        let repo_path = project.repo_path();
        if !repo_path.exists() {
            continue;
        }
        report.total = report.total.saturating_add(1);
        match write_project_mcp_config(&repo_path, new_token, port) {
            Ok(()) => {
                // Reaffirm the gitignore line in case the user (or a teammate's
                // merge) removed it. Without this, a rotated token could land
                // in source control on the next `git add -A`.
                if let Err(e) = append_to_gitignore(&repo_path, MCP_CONFIG_REL) {
                    tracing::warn!(
                        project_slug = %slug,
                        error = %e,
                        "cannot reaffirm .gitignore during token sweep; token \
                         file is still mode 0600 but a future commit could \
                         track it"
                    );
                }
                report.updated = report.updated.saturating_add(1);
            }
            Err(e) => {
                tracing::warn!(
                    project_slug = %slug,
                    error = %e,
                    "cannot rewrite .cursor/mcp.json during token sweep"
                );
                report.failed.push((slug.to_string(), e.to_string()));
            }
        }
    }
    Ok(report)
}

/// Read existing config and ensure it deserialises to a JSON object. A
/// missing or empty file resolves to an empty object so the merge path is
/// uniform: editors (and users) routinely leave a zero-byte `mcp.json`
/// behind, and that is not a parse error the caller should have to fix by
/// hand.
fn read_existing_config(path: &Path) -> Result<Value, AppError> {
    match std::fs::read_to_string(path) {
        Ok(raw) if raw.trim().is_empty() => Ok(Value::Object(serde_json::Map::new())),
        Ok(raw) => {
            let value: Value = serde_json::from_str(&raw).map_err(|e| AppError::StorageFailed {
                reason: format!(
                    "cannot parse {}: {e}; remove or fix the file before retrying",
                    path.display()
                ),
            })?;
            if matches!(value, Value::Object(_)) {
                Ok(value)
            } else {
                Err(AppError::StorageFailed {
                    reason: format!(
                        "cannot merge {}: top-level JSON must be an object",
                        path.display()
                    ),
                })
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(Value::Object(serde_json::Map::new()))
        }
        Err(e) => Err(e.into()),
    }
}

/// Write or update the `mcpServers.opnble` entry in `root` with the current
/// token + port. Other `mcpServers.*` entries are preserved.
fn set_opnble_entry(root: &mut Value, token: &str, port: u16) -> Result<(), AppError> {
    let Some(obj) = root.as_object_mut() else {
        return Err(AppError::StorageFailed {
            reason: "cannot update mcp config: top-level value is not an object".to_string(),
        });
    };
    let servers_entry = obj
        .entry(MCP_SERVERS_KEY.to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let Some(servers) = servers_entry.as_object_mut() else {
        return Err(AppError::StorageFailed {
            reason: format!("cannot update mcp config: '{MCP_SERVERS_KEY}' must be an object"),
        });
    };
    servers.insert(SERVER_NAME.to_string(), opnble_entry(token, port));
    Ok(())
}

/// Build the `mcpServers.opnble` value: a remote streamable-HTTP MCP server
/// keyed off the bearer token. Cursor and other rmcp-aware clients accept
/// the `url` + `headers` shape.
fn opnble_entry(token: &str, port: u16) -> Value {
    json!({
        "url": format!("http://127.0.0.1:{port}{path}", path = config::MCP_PATH),
        "headers": {
            "Authorization": format!("Bearer {token}"),
        },
    })
}

/// Pretty-print JSON with two-space indent and a trailing newline so the
/// file diffs cleanly if the user inspects it. The file carries the bearer
/// token, so it gets the same mode-0600 hardening as `~/.opnble/mcp.json`
/// on Unix; Windows ACLs depend on the parent directory and are not
/// tightened here (see threat-model notes in `mcp::auth`).
fn write_pretty(path: &Path, value: &Value) -> Result<(), AppError> {
    let mut body = serde_json::to_string_pretty(value)?;
    body.push('\n');
    std::fs::write(path, body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Return true if `body` already lists `line` as a non-comment, non-blank
/// `.gitignore` entry. Trims whitespace before comparing to tolerate the
/// usual one-line-per-pattern style.
fn gitignore_contains(body: &str, line: &str) -> bool {
    body.lines().any(|raw| {
        let trimmed = raw.trim();
        !trimmed.is_empty() && !trimmed.starts_with('#') && trimmed == line
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::types::{Project, ProjectId};
    use tempfile::TempDir;

    /// Local alias so tests stay readable after the `pub const` rename.
    const GITIGNORE_LINE: &str = MCP_CONFIG_REL;

    fn read_json(path: &Path) -> Value {
        let raw = std::fs::read_to_string(path).expect("read config");
        serde_json::from_str(&raw).expect("parse config")
    }

    #[cfg(unix)]
    #[test]
    fn project_mcp_json_is_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().expect("tempdir");
        write_project_mcp_config(dir.path(), "deadbeef", 47821).expect("write");
        let path = dir.path().join(".cursor").join("mcp.json");
        let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "per-project mcp.json carries the bearer token; got mode {mode:o}"
        );
    }

    #[test]
    fn write_creates_file_when_absent() {
        let dir = TempDir::new().expect("tempdir");
        write_project_mcp_config(dir.path(), "deadbeef", 47821).expect("write");

        let path = dir.path().join(".cursor").join("mcp.json");
        assert!(path.exists(), "config should exist on disk");

        let value = read_json(&path);
        let entry = value
            .get("mcpServers")
            .and_then(|v| v.get("opnble"))
            .expect("opnble entry");
        assert_eq!(
            entry.get("url").and_then(Value::as_str),
            Some(format!("http://127.0.0.1:47821{}", config::MCP_PATH).as_str())
        );
        let auth = entry
            .get("headers")
            .and_then(|h| h.get("Authorization"))
            .and_then(Value::as_str)
            .expect("auth header");
        assert_eq!(auth, "Bearer deadbeef");
    }

    #[test]
    fn write_preserves_other_servers() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join(".cursor").join("mcp.json");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");

        let initial = json!({
            "mcpServers": {
                "foo": {
                    "command": "echo",
                    "args": ["hello"],
                }
            },
            "experiments": { "newToolFormat": true }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).expect("ser")).expect("seed");

        write_project_mcp_config(dir.path(), "newtoken", 47821).expect("write");

        let merged = read_json(&path);
        let foo = merged
            .get("mcpServers")
            .and_then(|v| v.get("foo"))
            .expect("foo preserved");
        assert_eq!(
            foo.get("command").and_then(Value::as_str),
            Some("echo"),
            "user-added foo entry should survive merge"
        );
        assert_eq!(
            merged
                .get("experiments")
                .and_then(|e| e.get("newToolFormat"))
                .and_then(Value::as_bool),
            Some(true),
            "unrelated top-level keys should survive merge"
        );
        let opnble_token = merged
            .get("mcpServers")
            .and_then(|v| v.get("opnble"))
            .and_then(|o| o.get("headers"))
            .and_then(|h| h.get("Authorization"))
            .and_then(Value::as_str)
            .expect("auth");
        assert_eq!(opnble_token, "Bearer newtoken");
    }

    #[test]
    fn write_updates_existing_opnble_entry() {
        let dir = TempDir::new().expect("tempdir");
        write_project_mcp_config(dir.path(), "first", 47821).expect("seed");
        write_project_mcp_config(dir.path(), "second", 47821).expect("rotate");

        let value = read_json(&dir.path().join(".cursor").join("mcp.json"));
        let auth = value
            .get("mcpServers")
            .and_then(|v| v.get("opnble"))
            .and_then(|o| o.get("headers"))
            .and_then(|h| h.get("Authorization"))
            .and_then(Value::as_str)
            .expect("auth");
        assert_eq!(auth, "Bearer second");
    }

    #[test]
    fn write_returns_err_when_existing_file_invalid() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join(".cursor").join("mcp.json");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, b"{ not json").expect("seed");

        let err = write_project_mcp_config(dir.path(), "token", 47821)
            .expect_err("invalid json should error");
        assert!(matches!(err, AppError::StorageFailed { .. }));
    }

    #[test]
    fn write_treats_empty_file_as_fresh_config() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join(".cursor").join("mcp.json");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, b"").expect("seed empty file");

        write_project_mcp_config(dir.path(), "token", 47821)
            .expect("empty file should merge, not error");

        let value = read_json(&path);
        let auth = value
            .get("mcpServers")
            .and_then(|v| v.get("opnble"))
            .and_then(|o| o.get("headers"))
            .and_then(|h| h.get("Authorization"))
            .and_then(Value::as_str)
            .expect("auth header");
        assert_eq!(auth, "Bearer token");
    }

    #[test]
    fn write_treats_whitespace_only_file_as_fresh_config() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join(".cursor").join("mcp.json");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, b"  \n\t\n").expect("seed whitespace file");

        write_project_mcp_config(dir.path(), "token", 47821)
            .expect("whitespace-only file should merge, not error");

        let value = read_json(&path);
        assert!(
            value
                .get("mcpServers")
                .and_then(|v| v.get("opnble"))
                .is_some(),
            "opnble entry should be written into a whitespace-only file"
        );
    }

    #[test]
    fn write_returns_err_when_existing_root_not_object() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join(".cursor").join("mcp.json");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, b"[\"unexpected\"]").expect("seed");

        let err = write_project_mcp_config(dir.path(), "token", 47821)
            .expect_err("non-object root should error");
        assert!(matches!(err, AppError::StorageFailed { .. }));
    }

    #[test]
    fn write_emits_trailing_newline() {
        let dir = TempDir::new().expect("tempdir");
        write_project_mcp_config(dir.path(), "tok", 47821).expect("write");
        let body =
            std::fs::read_to_string(dir.path().join(".cursor").join("mcp.json")).expect("read");
        assert!(body.ends_with('\n'));
    }

    #[test]
    fn append_creates_gitignore_when_absent() {
        let dir = TempDir::new().expect("tempdir");
        append_to_gitignore(dir.path(), GITIGNORE_LINE).expect("append");
        let body = std::fs::read_to_string(dir.path().join(".gitignore")).expect("read");
        assert_eq!(body, format!("{GITIGNORE_LINE}\n"));
    }

    #[test]
    fn append_is_noop_when_line_already_present() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join(".gitignore");
        std::fs::write(&path, format!("node_modules\n{GITIGNORE_LINE}\n")).expect("seed");
        let before = std::fs::read_to_string(&path).expect("read");
        append_to_gitignore(dir.path(), GITIGNORE_LINE).expect("append");
        let after = std::fs::read_to_string(&path).expect("read");
        assert_eq!(before, after);
    }

    #[test]
    fn append_inserts_leading_newline_when_file_lacks_one() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join(".gitignore");
        std::fs::write(&path, b"node_modules").expect("seed");
        append_to_gitignore(dir.path(), GITIGNORE_LINE).expect("append");
        let body = std::fs::read_to_string(&path).expect("read");
        assert_eq!(body, format!("node_modules\n{GITIGNORE_LINE}\n"));
    }

    #[test]
    fn append_ignores_commented_match() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join(".gitignore");
        std::fs::write(&path, format!("# {GITIGNORE_LINE}\n")).expect("seed");
        append_to_gitignore(dir.path(), GITIGNORE_LINE).expect("append");
        let body = std::fs::read_to_string(&path).expect("read");
        assert_eq!(body, format!("# {GITIGNORE_LINE}\n{GITIGNORE_LINE}\n"));
    }

    /// Tiny in-memory store for the sweep test. `ProjectStore::list` only
    /// returns a clone, and the mutating trait methods are unreachable
    /// from these tests (they fail loudly via `unused`), so plain `Vec`
    /// without interior mutability is sufficient.
    struct MockStore {
        projects: Vec<Project>,
    }

    impl MockStore {
        fn with(projects: Vec<Project>) -> Self {
            Self { projects }
        }
    }

    fn unused(method: &str) -> AppError {
        AppError::Internal {
            reason: format!("MockStore::{method} unused in install tests"),
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

    #[test]
    fn sweep_token_counts_updates_and_skips_legacy() {
        let dir = TempDir::new().expect("tempdir");
        let repo_a = dir.path().join("a");
        let repo_b = dir.path().join("b");
        std::fs::create_dir_all(&repo_a).expect("mkdir a");
        std::fs::create_dir_all(&repo_b).expect("mkdir b");

        let store = MockStore::with(vec![
            project_at("p1", Some("alpha"), &repo_a),
            project_at("p2", Some("beta"), &repo_b),
            project_at("p3", None, &repo_b),
            project_at("p4", Some("gone"), &dir.path().join("missing")),
        ]);

        let report = sweep_token_with_global(&store, "rotated", 47821, None).expect("sweep");
        assert_eq!(report.total, 2, "only existing+slugged projects count");
        assert_eq!(report.updated, 2);
        assert!(report.failed.is_empty(), "no failures expected");

        for repo in [&repo_a, &repo_b] {
            let value = read_json(&repo.join(".cursor").join("mcp.json"));
            let auth = value
                .get("mcpServers")
                .and_then(|v| v.get("opnble"))
                .and_then(|o| o.get("headers"))
                .and_then(|h| h.get("Authorization"))
                .and_then(Value::as_str)
                .expect("auth header");
            assert_eq!(auth, "Bearer rotated");
        }
    }

    #[test]
    fn global_config_path_in_returns_none_when_cursor_dir_absent() {
        let dir = TempDir::new().expect("tempdir");
        assert!(
            global_config_path_in(dir.path()).is_none(),
            "no ~/.cursor/ on disk should resolve to None so we never create the dir"
        );
    }

    #[test]
    fn global_config_path_in_returns_some_when_cursor_dir_exists() {
        let dir = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(".cursor")).expect("mkdir cursor");
        let resolved = global_config_path_in(dir.path()).expect("path");
        assert_eq!(resolved, dir.path().join(".cursor").join("mcp.json"));
    }

    #[test]
    fn write_mcp_config_at_creates_parent_dirs() {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("nested").join("deeper").join("mcp.json");
        write_mcp_config_at(&path, "tok", 47821).expect("write");
        assert!(path.exists(), "deep parents should be created");
        let value = read_json(&path);
        let auth = value
            .get("mcpServers")
            .and_then(|v| v.get("opnble"))
            .and_then(|o| o.get("headers"))
            .and_then(|h| h.get("Authorization"))
            .and_then(Value::as_str)
            .expect("auth header");
        assert_eq!(auth, "Bearer tok");
    }

    #[test]
    fn sweep_with_global_rewrites_global_and_projects_in_lockstep() {
        let dir = TempDir::new().expect("tempdir");
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).expect("mkdir repo");
        let global = dir.path().join("home").join(".cursor").join("mcp.json");

        let store = MockStore::with(vec![project_at("p1", Some("alpha"), &repo)]);

        let report = sweep_token_with_global(&store, "rotated", 47821, Some(&global))
            .expect("sweep with global");
        assert_eq!(report.total, 1);
        assert_eq!(report.updated, 1);
        assert!(report.failed.is_empty());

        for path in [global, repo.join(".cursor").join("mcp.json")] {
            let value = read_json(&path);
            let auth = value
                .get("mcpServers")
                .and_then(|v| v.get("opnble"))
                .and_then(|o| o.get("headers"))
                .and_then(|h| h.get("Authorization"))
                .and_then(Value::as_str)
                .expect("auth header missing in sweep-rewritten mcp.json");
            assert_eq!(
                auth,
                "Bearer rotated",
                "global and per-project share one token; mismatch at {}",
                path.display(),
            );
        }
    }

    #[test]
    fn sweep_with_invalid_global_logs_and_still_processes_projects() {
        let dir = TempDir::new().expect("tempdir");
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).expect("mkdir repo");
        let global = dir.path().join("home").join(".cursor").join("mcp.json");
        std::fs::create_dir_all(global.parent().expect("parent")).expect("mkdir cursor");
        std::fs::write(&global, b"{ not json").expect("seed invalid global");

        let store = MockStore::with(vec![project_at("p1", Some("alpha"), &repo)]);
        let report = sweep_token_with_global(&store, "rotated", 47821, Some(&global))
            .expect("sweep must not bubble global failure");
        assert_eq!(
            report.total, 1,
            "project sweep still runs after global failure"
        );
        assert_eq!(report.updated, 1, "project file is still updated");
    }

    #[test]
    fn sweep_token_records_failure_when_existing_invalid() {
        let dir = TempDir::new().expect("tempdir");
        let repo = dir.path().join("a");
        std::fs::create_dir_all(repo.join(".cursor")).expect("mkdir");
        std::fs::write(repo.join(".cursor").join("mcp.json"), b"{ broken").expect("seed");

        let store = MockStore::with(vec![project_at("p1", Some("alpha"), &repo)]);
        let report = sweep_token_with_global(&store, "rotated", 47821, None).expect("sweep");
        assert_eq!(report.total, 1);
        assert_eq!(report.updated, 0);
        assert_eq!(report.failed.len(), 1);
        let (slug, _) = report.failed.first().expect("failure recorded");
        assert_eq!(slug, "alpha");
    }
}
