//! View types returned from MCP tool bodies to clients.
//!
//! `Project` is the on-disk shape and contains fields agents do not need
//! (intent, `local_path`, machine-local id). The view types here pick the
//! agent-facing subset and serialize as `camelCase` so the JSON shape
//! matches what the MCP plan documents.
//!
//! `from_project_with_slug` takes the unwrapped slug as a separate argument
//! so the type cannot represent the impossible "project view without a
//! resolvable slug" state. Callers in `tools/discovery.rs` filter projects
//! whose `slug` is `None` before constructing.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::projects::types::Project;

/// Agent-facing summary of a project. Returned by `list_projects` (one per
/// installed project with a slug) and `get_project` (single).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
    pub slug: String,
    pub name: String,
    pub id: String,
    pub repo: String,
    pub branch: String,
    pub status: String,
    pub port: Option<u16>,
    pub preview_url: Option<String>,
    pub pages_url: Option<String>,
    pub last_synced_at: u64,
}

/// Object wrapper for `list_projects`. rmcp 1.7.0 rejects tool output schemas
/// whose root is an array, so the project list ships under a `projects` key.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectListView {
    pub projects: Vec<ProjectView>,
}

impl ProjectView {
    /// Use after the caller has resolved the slug (typically via
    /// `ProjectStore::get_by_slug` or by filtering out `slug.is_none()`
    /// projects). Taking the slug as a separate arg makes the "must have
    /// a slug" precondition explicit at the type level.
    pub fn from_project_with_slug(project: &Project, slug: &str) -> Self {
        let preview_url = project.port.map(|p| format!("http://127.0.0.1:{p}/"));
        Self {
            slug: slug.to_string(),
            name: project.name.clone(),
            id: project.id.as_str().to_string(),
            repo: project.repo_url.clone(),
            branch: project.branch.clone(),
            status: project.status.to_string(),
            port: project.port,
            preview_url,
            pages_url: project.pages_url.clone(),
            last_synced_at: project.last_synced_at,
        }
    }
}

/// Result of `start_project` / `restart_project`. The agent typically wants
/// the preview URL it can hit immediately, so build it server-side.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunningView {
    pub status: String,
    pub port: u16,
    pub url: String,
}

impl RunningView {
    pub fn for_port(port: u16) -> Self {
        Self {
            status: "running".to_string(),
            port,
            url: format!("http://127.0.0.1:{port}/"),
        }
    }
}

/// Result of `stop_project`. Status only (no preview URL because there is
/// nothing to preview).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StatusView {
    pub status: String,
}

impl StatusView {
    pub fn stopped() -> Self {
        Self {
            status: "stopped".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileArgs {
    pub slug: String,
    pub path: String,
    #[serde(default = "default_read_max_bytes")]
    pub max_bytes: u64,
}

const fn default_read_max_bytes() -> u64 {
    1_048_576 // 1 MiB
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileView {
    pub content: String,
    pub encoding: String,
    pub size: u64,
    pub truncated: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileArgs {
    pub slug: String,
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub encoding: Option<String>,
    /// Wire-level opt-out for parent-directory creation. The current impl
    /// always creates parents via `safe_join_within_repo(must_exist=false)`,
    /// which always runs `create_dir_all` on the canonical parent. The field
    /// is kept on the schema so the contract stays stable; honouring
    /// `create_parents: false` is a follow-up if a caller ever needs it.
    #[allow(dead_code, reason = "schema-only field; impl always creates parents")]
    #[serde(default = "default_create_parents")]
    pub create_parents: bool,
}

const fn default_create_parents() -> bool {
    true
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileView {
    pub bytes_written: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecCommandArgs {
    pub slug: String,
    pub cmd: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    #[serde(default = "default_exec_timeout_ms")]
    pub timeout_ms: u64,
}

const fn default_exec_timeout_ms() -> u64 {
    60_000
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecCommandView {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub truncated: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotArgs {
    pub slug: String,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotView {
    pub snapshot_id: String,
    pub label: Option<String>,
    pub taken_at: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RollbackArgs {
    pub slug: String,
    pub snapshot_id: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RollbackView {
    pub snapshot_id: String,
    pub restored_at: u64,
    pub interruption_ms: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ForkArgs {
    pub slug: String,
    #[serde(default = "default_fork_count")]
    pub count: u32,
    #[allow(
        dead_code,
        reason = "accepted in the schema; v1.1 does not surface label yet (see v1.2)"
    )]
    #[serde(default)]
    pub label: Option<String>,
}

const fn default_fork_count() -> u32 {
    1
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ForkChild {
    pub slug: String,
    pub parent_slug: String,
    pub status: String,
    pub port: Option<u16>,
    pub url: Option<String>,
}

/// Object wrapper for `fork_project`. rmcp 1.7.0 rejects tool output schemas
/// whose root is an array, so the fork children ship under a `children` key.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ForkChildrenView {
    pub children: Vec<ForkChild>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::types::{ProjectId, ProjectIntent, ProjectStatus};

    fn project_with(status: ProjectStatus, port: Option<u16>) -> Project {
        Project {
            id: ProjectId::new("proj-1").expect("valid id"),
            name: "My Blog".to_string(),
            slug: Some("my-blog".to_string()),
            repo_url: "https://github.com/me/blog".to_string(),
            local_path: "/tmp/proj-1".to_string(),
            status,
            intent: ProjectIntent::Run,
            port,
            branch: "main".to_string(),
            last_synced_at: 1_700_000_000,
            tunnel_id: None,
            tunnel_url: None,
            pages_url: Some("https://me.github.io/blog".to_string()),
            installed: true,
            parent_project_id: None,
            forked_at: None,
        }
    }

    #[test]
    fn project_view_round_trips_fields() {
        let project = project_with(ProjectStatus::Ready, Some(3005));
        let view = ProjectView::from_project_with_slug(&project, "my-blog");
        assert_eq!(view.slug, "my-blog");
        assert_eq!(view.name, "My Blog");
        assert_eq!(view.id, "proj-1");
        assert_eq!(view.repo, "https://github.com/me/blog");
        assert_eq!(view.branch, "main");
        assert_eq!(view.status, "ready");
        assert_eq!(view.port, Some(3005));
        assert_eq!(view.preview_url.as_deref(), Some("http://127.0.0.1:3005/"));
        assert_eq!(view.pages_url.as_deref(), Some("https://me.github.io/blog"));
        assert_eq!(view.last_synced_at, 1_700_000_000);
    }

    #[test]
    fn project_view_serializes_status_as_camel_case_string() {
        let project = project_with(ProjectStatus::WaitingForVm, None);
        let view = ProjectView::from_project_with_slug(&project, "my-blog");
        assert_eq!(view.status, "waitingForVm");
        assert!(view.preview_url.is_none());
    }

    #[test]
    fn project_view_omits_preview_url_when_no_port() {
        let project = project_with(ProjectStatus::Stopped, None);
        let view = ProjectView::from_project_with_slug(&project, "my-blog");
        assert!(view.preview_url.is_none());
        assert!(view.port.is_none());
    }

    #[test]
    fn running_view_carries_url_built_from_port() {
        let view = RunningView::for_port(3007);
        assert_eq!(view.status, "running");
        assert_eq!(view.port, 3007);
        assert_eq!(view.url, "http://127.0.0.1:3007/");
    }

    #[test]
    fn status_view_stopped_carries_lowercase_status() {
        let view = StatusView::stopped();
        assert_eq!(view.status, "stopped");
    }

    #[test]
    fn project_view_serializes_camel_case_json() {
        let project = project_with(ProjectStatus::Ready, Some(3005));
        let view = ProjectView::from_project_with_slug(&project, "my-blog");
        let json = serde_json::to_value(&view).expect("serialize");
        let obj = json.as_object().expect("object");
        assert!(obj.contains_key("lastSyncedAt"));
        assert!(obj.contains_key("previewUrl"));
        assert!(obj.contains_key("pagesUrl"));
        assert!(!obj.contains_key("last_synced_at"));
    }

    #[test]
    fn read_file_args_default_max_bytes_is_one_mib() {
        let json = r#"{"slug":"blog","path":"src/index.ts"}"#;
        let args: ReadFileArgs = serde_json::from_str(json).unwrap();
        assert_eq!(args.max_bytes, 1_048_576);
    }

    #[test]
    fn write_file_view_serializes_camel_case() {
        let view = WriteFileView { bytes_written: 42 };
        let json = serde_json::to_string(&view).unwrap();
        assert_eq!(json, r#"{"bytesWritten":42}"#);
    }

    #[test]
    fn exec_command_args_default_timeout_is_60s() {
        let json = r#"{"slug":"blog","cmd":"ls"}"#;
        let args: ExecCommandArgs = serde_json::from_str(json).unwrap();
        assert_eq!(args.timeout_ms, 60_000);
    }

    #[test]
    fn exec_command_view_serializes_camel_case() {
        let view = ExecCommandView {
            stdout: "ok".into(),
            stderr: String::new(),
            exit_code: 0,
            truncated: false,
            duration_ms: 12,
        };
        let json = serde_json::to_string(&view).unwrap();
        assert!(json.contains("\"exitCode\":0"));
        assert!(json.contains("\"durationMs\":12"));
    }

    #[test]
    fn snapshot_view_serializes_camel_case() {
        let v = SnapshotView {
            snapshot_id: "snap-1-aaaa".into(),
            label: None,
            taken_at: 1,
        };
        assert!(serde_json::to_string(&v)
            .unwrap()
            .contains("\"snapshotId\""));
    }

    #[test]
    fn fork_args_default_count_is_one() {
        let json = r#"{"slug":"blog"}"#;
        let args: ForkArgs = serde_json::from_str(json).unwrap();
        assert_eq!(args.count, 1);
        assert!(args.label.is_none());
    }

    #[test]
    fn fork_child_serializes_camel_case() {
        let child = ForkChild {
            slug: "blog-abc123".into(),
            parent_slug: "blog".into(),
            status: "ready".into(),
            port: Some(3002),
            url: Some("http://127.0.0.1:3002/".into()),
        };
        let json = serde_json::to_string(&child).unwrap();
        assert!(json.contains("\"parentSlug\":\"blog\""));
        assert!(json.contains("\"port\":3002"));
    }
}
