//! MCP tool bodies: discovery and lifecycle.
//!
//! All Phase 2 tools live in one `#[tool_router(vis = "pub(crate)")]` impl
//! block on [`OpnbleMcp`]. rmcp emits a single `tool_router()` fn per impl
//! block, so splitting tools across files would require per-file routers
//! and merging at startup. One block keeps the wire-up simple and the
//! diff small.
//!
//! Phases 3 onward (`tail_logs`, git tools, share/unshare, publish, VM)
//! extend the same impl block here. Despite the file name, this is the
//! single home for every MCP tool body.

use std::path::Path;
use std::time::Duration;

use rmcp::{
    handler::server::wrapper::{Json, Parameters},
    model::{ErrorCode, ErrorData as McpError},
    schemars, tool, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::infrastructure::git::{FileStatus, PushOutcome, RepoStatus};
use crate::mcp::errors::{into_mcp, no_github_auth, no_remote, publish_timeout, push_failed};
use crate::mcp::resolve::project_for_slug;
use crate::mcp::server::OpnbleMcp;
use crate::mcp::tools::types::{
    ExecCommandArgs, ExecCommandView, ForkArgs, ForkChild, ForkChildrenView, ProjectListView,
    ProjectView, ReadFileArgs, ReadFileView, RollbackArgs, RollbackView, RunningView, SnapshotArgs,
    SnapshotView, StatusView, WriteFileArgs, WriteFileView,
};
use crate::mcp::wait::{wait_for_status, WaitOutcome};
use crate::projects::types::{unix_now_millis, Project, ProjectId, ProjectStatus};
use crate::publish::deployer::{self, PublishDeps, UnpublishDeps};
use crate::publish::types::Framework;
use crate::tunnel::TunnelCoordinator;

/// JSON-RPC "Invalid params" code, used for caller-fixable errors like
/// "project not installed" or "project not found".
const INVALID_PARAMS: ErrorCode = ErrorCode(-32602);

/// JSON-RPC "Internal error" code, used for runtime failures like
/// container start failure or wait timeout.
const INTERNAL_ERROR: ErrorCode = ErrorCode(-32603);

/// Hard ceiling for how long `start_project` blocks waiting for `Ready`.
/// First-run installs (npm install + dev server boot) can run 30-90 seconds;
/// 300s gives slow networks plenty of headroom while still failing fast
/// enough that an agent can fall back to manual triage.
const START_DEADLINE: Duration = Duration::from_mins(5);

/// Hard ceiling for how long `stop_project` blocks waiting for `Stopped`.
/// Container stop is fast (SIGTERM + 10s grace, then SIGKILL); 120s is
/// generous insurance against a hung dev server.
const STOP_DEADLINE: Duration = Duration::from_mins(2);

/// Default `tail_logs` request size when the caller omits `lines`. Sized to
/// fit a typical agent context window without blowing past the per-call cap.
const DEFAULT_TAIL_LINES: u32 = 200;

/// Hard ceiling on a single `tail_logs` request. Bounds how much log text a
/// runaway agent can pull in one call; agents that need more should page by
/// calling `tail_logs` repeatedly with smaller increments.
const MAX_TAIL_LINES: u32 = 5_000;

/// Arguments for [`OpnbleMcp::get_project`].
///
/// `slug` is the portable identifier persisted to `state.json` and the
/// future `.opnble` repo marker, not the local `proj-{millis}` id.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetProjectArgs {
    pub slug: String,
}

/// Arguments for the lifecycle tools (`start_project`, `stop_project`,
/// `restart_project`). All three only need a slug today; if Phase 3+ adds
/// per-tool flags they get their own argument structs.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SlugOnly {
    pub slug: String,
}

// === Phase 3: logs arg / view types ===

/// Arguments for [`OpnbleMcp::tail_logs`].
///
/// `lines` is optional so the common "give me the recent tail" call needs no
/// arguments beyond the slug; defaults and caps are enforced in
/// [`cap_lines`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TailLogsArgs {
    pub slug: String,
    /// Number of trailing lines to return. Default 200, capped at 5000.
    #[serde(default)]
    pub lines: Option<u32>,
}

/// Result of [`OpnbleMcp::tail_logs`]. `lines` is the most recent tail in
/// chronological order (oldest first), matching the underlying container
/// engine's output.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TailLogsView {
    pub lines: Vec<String>,
    /// True when the returned vec hit the per-call cap, signalling that
    /// older lines exist beyond the window. Best-effort: container engines
    /// do not report whether more history exists, so this can also be true
    /// when the container produced exactly `lines` output lines total.
    pub truncated: bool,
}

// === Phase 4: git arg / view types ===

/// Arguments for [`OpnbleMcp::switch_branch`].
///
/// `create: true` creates the branch from the current HEAD before checking
/// it out. The default `create: false` requires the branch to already exist
/// locally so the agent doesn't accidentally branch off the wrong commit.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SwitchBranchArgs {
    pub slug: String,
    pub branch: String,
    #[serde(default)]
    pub create: bool,
}

/// Porcelain working-tree status returned to MCP clients. Mirrors
/// [`RepoStatus`] but with `ahead`/`behind` narrowed to `u32` (json schema
/// safe) and the `FileChange` enum projected to a stable lowercase string so
/// agents don't need to know the Rust variant names.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusView {
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub dirty: bool,
    pub files: Vec<FileStatusView>,
}

/// One changed path inside [`GitStatusView::files`].
///
/// `status` is the same string serde produces for [`crate::infrastructure::git::FileChange`]
/// (`"added"`, `"modified"`, `"deleted"`, `"untracked"`, `"renamed"`), kept
/// in sync via `FileChange::as_str`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileStatusView {
    pub path: String,
    pub status: String,
}

/// Result of `switch_branch`: the branch name now checked out.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BranchView {
    pub branch: String,
}

/// Result of `pull_latest`: the current branch plus ahead/behind counters
/// taken from a fresh `status()` after the fast-forward attempt.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PullView {
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
}

impl GitStatusView {
    /// Project a [`RepoStatus`] onto the agent-facing view shape.
    ///
    /// `ahead`/`behind` come back as `usize` from libgit2 and are saturating-
    /// converted to `u32` (the maximum ahead/behind we report). Conversion
    /// uses `try_from` because the workspace denies `as` casts; in the
    /// unrealistic case where a single branch is more than 4 billion commits
    /// out of sync we clamp to `u32::MAX` so the agent still sees "lots".
    pub fn from_repo_status(status: &RepoStatus) -> Self {
        Self {
            branch: status.branch.clone(),
            ahead: u32::try_from(status.ahead).unwrap_or(u32::MAX),
            behind: u32::try_from(status.behind).unwrap_or(u32::MAX),
            dirty: status.dirty,
            files: status.files.iter().map(FileStatusView::from_file).collect(),
        }
    }
}

impl FileStatusView {
    /// Build the wire view for a single file status, mapping `FileChange`
    /// through its stable string discriminator.
    pub fn from_file(file: &FileStatus) -> Self {
        Self {
            path: file.path.clone(),
            status: file.status.as_str().to_string(),
        }
    }
}

/// Active preview-link state surfaced by the Phase 5 tools.
///
/// `live` reflects the cloudflared "Registered tunnel connection" signal:
/// `false` immediately after `share` until the watcher observes the edge
/// connection, then `true` for the rest of the tunnel's lifetime.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreviewView {
    pub url: String,
    pub live: bool,
}

/// Object wrapper for `get_preview_link`. rmcp 1.7.0 requires an object-rooted
/// output schema, so the optional preview ships under a nullable `preview` key:
/// `null` means no tunnel is registered, a present value may still be offline.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreviewLinkView {
    pub preview: Option<PreviewView>,
}

/// Result of `stop_preview_link`. Single boolean field rather than a bare
/// `bool` so future fields (last URL, cleanup warnings) can be added
/// without breaking the JSON contract.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StoppedView {
    pub stopped: bool,
}

// === Phase 6 args + views ===

/// Arguments for [`OpnbleMcp::github_push`].
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GithubPushArgs {
    pub slug: String,
    pub message: String,
    /// When `true` (default), creates a fresh GitHub repo if the worktree has
    /// no `origin` remote. Set to `false` to fail fast for projects you want
    /// to wire up manually.
    #[serde(default = "default_true")]
    pub create_repo_if_missing: bool,
    /// Visibility of the new repo when `create_repo_if_missing` fires.
    /// Defaults to `Private` (no contents leak if the push fails).
    #[serde(default)]
    pub visibility: PushVisibility,
}

const fn default_true() -> bool {
    true
}

/// Visibility for repos created via the `create_repo_if_missing` flow.
///
/// Lower-case names match the on-the-wire convention shared with the
/// frontend (`"public"` / `"private"`).
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PushVisibility {
    Public,
    #[default]
    Private,
}

/// Result of [`OpnbleMcp::github_push`].
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PushView {
    /// Full clone URL of `origin`, post-push. Either the existing remote URL
    /// or, when `created_repo == true`, the URL of the freshly minted repo.
    pub remote_url: String,
    /// 40-char hex SHA of the commit at `HEAD` after the push.
    pub commit_sha: String,
    /// `true` when the push flow created a new GitHub repo to host the
    /// project, `false` when it pushed to an existing remote.
    pub created_repo: bool,
}

/// Result of [`OpnbleMcp::github_pages_publish`].
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishedView {
    pub url: String,
    /// Lower-camel-case framework label (`"nextJs"`, `"vite"`, `"cra"`, or
    /// `"unknown"`). Mirrors the discriminant emitted by the publish module's
    /// own serializers so MCP clients see one consistent vocabulary.
    pub framework: String,
    /// Unix milliseconds at the moment Pages reported the deploy live.
    pub deployed_at: u64,
}

/// Result of [`OpnbleMcp::github_pages_unpublish`].
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnpublishedView {
    pub unpublished: bool,
}

/// Wall-clock ceiling for `github_pages_publish`: the build runs in a
/// container with no MCP-side cancellation surface, so we cap how long an
/// agent will block waiting for it. 600s matches the "10-minute ceiling"
/// shape Phase 6 asks for in the tool description.
const PUBLISH_DEADLINE: std::time::Duration = std::time::Duration::from_mins(10);

/// Stable wire label for a detected framework. Matches the camelCase
/// shape emitted by `Framework`'s own serializer so the MCP and Tauri
/// surfaces never disagree.
fn framework_label(framework: &Framework) -> &'static str {
    match framework {
        Framework::NextJs => "nextJs",
        Framework::Vite => "vite",
        Framework::Cra => "cra",
        Framework::Unknown => "unknown",
    }
}

#[tool_router(vis = "pub(crate)")]
impl OpnbleMcp {
    /// List every project Opnble knows about. Filters out projects whose
    /// slug has not been derived yet, since those are not addressable from
    /// the agent's perspective (no portable identifier).
    #[tool(
        description = "List all projects in Opnble",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    pub async fn list_projects(&self) -> Result<Json<ProjectListView>, McpError> {
        let result = self.list_projects_inner();
        self.record_outcome("list_projects", "", &result);
        result
    }

    fn list_projects_inner(&self) -> Result<Json<ProjectListView>, McpError> {
        let stored = self.store.list().map_err(into_mcp)?;
        let projects = stored
            .iter()
            .filter_map(project_view_if_addressable)
            .collect();
        Ok(Json(ProjectListView { projects }))
    }

    /// Look up a single project by its portable slug.
    ///
    /// The `get_` prefix here is the MCP tool name on the wire, not a Rust
    /// getter, so it stays despite the project's "no `get_` prefix on
    /// getters" naming rule. Same applies to `get_preview_link` below.
    #[tool(
        description = "Get details of a project by slug",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    pub async fn get_project(
        &self,
        Parameters(args): Parameters<GetProjectArgs>,
    ) -> Result<Json<ProjectView>, McpError> {
        let slug = args.slug.clone();
        let result = self.get_project_inner(&args);
        self.record_outcome("get_project", &slug, &result);
        result
    }

    fn get_project_inner(&self, args: &GetProjectArgs) -> Result<Json<ProjectView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        Ok(Json(project_view_for_resolved(&project, &args.slug)))
    }

    /// Start a project's dev server. Blocks until `Ready` is reached, the
    /// project transitions to `Failed`, or the start deadline elapses.
    #[tool(
        description = "Start a project's dev server. Blocks until the server is ready or fails.",
        annotations(destructive_hint = true, idempotent_hint = true)
    )]
    pub async fn start_project(
        &self,
        Parameters(args): Parameters<SlugOnly>,
    ) -> Result<Json<RunningView>, McpError> {
        let slug = args.slug.clone();
        let result = self.start_project_inner(args).await;
        self.record_outcome("start_project", &slug, &result);
        result
    }

    async fn start_project_inner(&self, args: SlugOnly) -> Result<Json<RunningView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        if !project.installed {
            return Err(not_installed_error(&args.slug));
        }
        if let Some(view) = idempotent_running_view(&project) {
            return Ok(Json(view));
        }
        let port = match project.port {
            Some(p) => p,
            None => self.store.allocate_port().map_err(into_mcp)?,
        };
        self.orchestrator
            .start_project(&self.app_handle, project.id.as_str(), port)
            .await
            .map_err(into_mcp)?;
        let view = self.await_ready(&args.slug, &project, port).await?;
        self.syncer
            .start_periodic(&self.app_handle, project.id.as_str())
            .await;
        Ok(view)
    }

    /// Stop a running project's dev server. Blocks until `Stopped`, the
    /// project transitions to `Failed`, or the stop deadline elapses.
    #[tool(
        description = "Stop a running project's dev server. Blocks until stopped.",
        annotations(destructive_hint = true, idempotent_hint = true)
    )]
    pub async fn stop_project(
        &self,
        Parameters(args): Parameters<SlugOnly>,
    ) -> Result<Json<StatusView>, McpError> {
        let slug = args.slug.clone();
        let result = self.stop_project_inner(args).await;
        self.record_outcome("stop_project", &slug, &result);
        result
    }

    async fn stop_project_inner(&self, args: SlugOnly) -> Result<Json<StatusView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        if project.status == ProjectStatus::Stopped {
            return Ok(Json(StatusView::stopped()));
        }
        // `stop_with_sync` wraps `orchestrator.stop_project` with the
        // periodic-worker shutdown and the on-stop commit + push the UI
        // path runs, so MCP `stop_project` produces the same side effects
        // as the Tauri lifecycle command.
        self.syncer
            .stop_with_sync(&self.app_handle, self.orchestrator.as_ref(), &project.id)
            .await
            .map_err(into_mcp)?;
        let outcome = wait_for_status(
            &self.app_handle,
            self.store.as_ref(),
            &project.id,
            ProjectStatus::Stopped,
            STOP_DEADLINE,
        )
        .await
        .map_err(into_mcp)?;
        match outcome {
            WaitOutcome::Ready(_) => Ok(Json(StatusView::stopped())),
            WaitOutcome::Failed(snap) => Err(stop_failed_error(&args.slug, snap.error_summary)),
            WaitOutcome::Timeout => Err(stop_timeout_error(&args.slug)),
        }
    }

    /// Restart a project: force-stop the container, allocate a fresh port,
    /// then start. Blocks until `Ready` or fails.
    #[tool(
        description = "Restart a project: stop, allocate fresh port, start. Blocks until ready.",
        annotations(destructive_hint = true, idempotent_hint = true)
    )]
    pub async fn restart_project(
        &self,
        Parameters(args): Parameters<SlugOnly>,
    ) -> Result<Json<RunningView>, McpError> {
        let slug = args.slug.clone();
        let result = self.restart_project_inner(args).await;
        self.record_outcome("restart_project", &slug, &result);
        result
    }

    async fn restart_project_inner(&self, args: SlugOnly) -> Result<Json<RunningView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        if !project.installed {
            return Err(not_installed_error(&args.slug));
        }
        // Mirror commands::restart_project: tear down the periodic worker
        // and the running container before allocating a fresh port + start.
        // We skip `stop_with_sync`'s commit + push here because restart is
        // expected to keep the dev server "live"; the periodic worker will
        // resume after start.
        self.syncer.stop_periodic(project.id.as_str()).await;
        self.orchestrator
            .force_stop_and_remove(project.id.as_str())
            .await
            .map_err(into_mcp)?;
        let port = self.store.allocate_port().map_err(into_mcp)?;
        self.orchestrator
            .start_project(&self.app_handle, project.id.as_str(), port)
            .await
            .map_err(into_mcp)?;
        let view = self.await_ready(&args.slug, &project, port).await?;
        self.syncer
            .start_periodic(&self.app_handle, project.id.as_str())
            .await;
        Ok(view)
    }

    /// Block on `wait_for_status(Ready)` and convert the outcome into a
    /// `RunningView` or an MCP error. Shared by `start_project` and
    /// `restart_project` so the failure mapping (and the log-tail fallback
    /// on timeout) lives in one place.
    async fn await_ready(
        &self,
        slug: &str,
        project: &Project,
        port: u16,
    ) -> Result<Json<RunningView>, McpError> {
        let outcome = wait_for_status(
            &self.app_handle,
            self.store.as_ref(),
            &project.id,
            ProjectStatus::Ready,
            START_DEADLINE,
        )
        .await
        .map_err(into_mcp)?;
        match outcome {
            WaitOutcome::Ready(snap) => Ok(Json(RunningView::for_port(snap.port.unwrap_or(port)))),
            WaitOutcome::Failed(snap) => Err(start_failed_error(slug, snap.error_summary)),
            WaitOutcome::Timeout => {
                let tail = self.last_log_line(project.id.as_str()).await;
                Err(start_timeout_error(slug, tail.as_deref()))
            }
        }
    }

    /// Best-effort fetch of the project container's last log line, used as
    /// the timeout-error hint. Failure is logged at debug level and the
    /// caller falls back to a generic timeout message.
    async fn last_log_line(&self, project_id: &str) -> Option<String> {
        match self.runtime.tail_logs(project_id, 1).await {
            Ok(mut lines) => lines.pop(),
            Err(e) => {
                tracing::debug!(
                    project_id = %project_id,
                    error = %e,
                    "cannot read container logs for timeout hint"
                );
                None
            }
        }
    }

    // === Phase 3: logs ===

    /// Return the most recent log lines from a project's container.
    ///
    /// The container engine is the source of truth: there is no app-side log
    /// buffer, so an empty `lines` vec means the container exists but has
    /// produced no output yet, not an error. Per-platform "container missing"
    /// semantics differ (Podman: `NotFound`; WSL2: `ContainerFailed`;
    /// `VmRuntime`: `VmNotRunning` if the VM is down) and are mapped to the
    /// right MCP code by [`crate::mcp::errors::into_mcp`].
    #[tool(
        description = "Return the most recent log lines from a project's container.",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    pub async fn tail_logs(
        &self,
        Parameters(args): Parameters<TailLogsArgs>,
    ) -> Result<Json<TailLogsView>, McpError> {
        let slug = args.slug.clone();
        let result = self.tail_logs_inner(args).await;
        self.record_outcome("tail_logs", &slug, &result);
        result
    }

    async fn tail_logs_inner(&self, args: TailLogsArgs) -> Result<Json<TailLogsView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        let requested = cap_lines(args.lines);
        let lines = self
            .runtime
            .tail_logs(project.id.as_str(), requested)
            .await
            .map_err(into_mcp)?;
        let truncated = u32::try_from(lines.len()).unwrap_or(u32::MAX) >= requested;
        Ok(Json(TailLogsView { lines, truncated }))
    }

    // === Phase 4: git ===

    /// Return the porcelain git status of a project's working tree.
    ///
    /// Read-only. Tolerates detached HEAD and unborn branches per
    /// `RepoStatus::branch`'s contract; for branches without an upstream the
    /// returned `ahead`/`behind` are both zero.
    #[tool(
        description = "Return the porcelain git status of a project's working tree.",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    pub async fn git_status(
        &self,
        Parameters(args): Parameters<SlugOnly>,
    ) -> Result<Json<GitStatusView>, McpError> {
        let slug = args.slug.clone();
        let result = self.git_status_inner(&args);
        self.record_outcome("git_status", &slug, &result);
        result
    }

    fn git_status_inner(&self, args: &SlugOnly) -> Result<Json<GitStatusView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        let repo_status = self.git.status(&project.repo_path()).map_err(into_mcp)?;
        Ok(Json(GitStatusView::from_repo_status(&repo_status)))
    }

    /// Check out a branch in a project's working tree. Set `create: true`
    /// to branch from the current HEAD before switching.
    ///
    /// With `create: true`, an existing branch surfaces as `ALREADY_EXISTS`.
    /// With `create: false`, a missing branch surfaces as `GIT_FAILED` with
    /// the libgit2 reason in `hint` (no separate "not found" remapping; the
    /// agent can read the hint to decide whether to retry with `create:
    /// true`).
    #[tool(
        description = "Check out a branch in a project's working tree. Set `create: true` to create from current HEAD.",
        annotations(destructive_hint = true)
    )]
    pub async fn switch_branch(
        &self,
        Parameters(args): Parameters<SwitchBranchArgs>,
    ) -> Result<Json<BranchView>, McpError> {
        let slug = args.slug.clone();
        let result = self.switch_branch_inner(args);
        self.record_outcome("switch_branch", &slug, &result);
        result
    }

    fn switch_branch_inner(&self, args: SwitchBranchArgs) -> Result<Json<BranchView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        self.git
            .checkout(&project.repo_path(), &args.branch, args.create)
            .map_err(into_mcp)?;
        Ok(Json(BranchView {
            branch: args.branch,
        }))
    }

    /// Fetch and fast-forward a project's current branch from `origin`.
    ///
    /// Reuses [`Authenticator::resolve_push_token`] because the fetch path
    /// goes through the same credentials surface; `Ok(None)` for non-GitHub
    /// hosts (the underlying libgit2 fetch then runs without credentials),
    /// and `AuthRequired { provider: "github" }` -> `NO_GITHUB_AUTH` for
    /// GitHub repos that have not been authenticated yet.
    #[tool(
        description = "Fetch and fast-forward a project's current branch from origin.",
        annotations(destructive_hint = true)
    )]
    pub async fn pull_latest(
        &self,
        Parameters(args): Parameters<SlugOnly>,
    ) -> Result<Json<PullView>, McpError> {
        let slug = args.slug.clone();
        let result = self.pull_latest_inner(args).await;
        self.record_outcome("pull_latest", &slug, &result);
        result
    }

    async fn pull_latest_inner(&self, args: SlugOnly) -> Result<Json<PullView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        let token = self
            .authenticator
            .resolve_push_token(&project.repo_url)
            .await
            .map_err(into_mcp)?;
        self.git
            .pull(&project.repo_path(), token.as_deref())
            .map_err(into_mcp)?;
        let status = self.git.status(&project.repo_path()).map_err(into_mcp)?;
        Ok(Json(PullView {
            branch: status.branch,
            ahead: u32::try_from(status.ahead).unwrap_or(u32::MAX),
            behind: u32::try_from(status.behind).unwrap_or(u32::MAX),
        }))
    }
    // === Phase 5: preview link ===

    /// Create or refresh a public preview link (Cloudflare Tunnel) for a
    /// running project. Idempotent: if a tunnel already exists for the
    /// project, [`TunnelCoordinator::share`] returns the cached URL without
    /// spawning another cloudflared.
    #[tool(
        description = "Create or refresh a public preview link (Cloudflare Tunnel) for a running project. Idempotent: returns the existing URL if one is already active.",
        annotations(destructive_hint = true, idempotent_hint = true)
    )]
    pub async fn create_preview_link(
        &self,
        Parameters(args): Parameters<SlugOnly>,
    ) -> Result<Json<PreviewView>, McpError> {
        let slug = args.slug.clone();
        let result = self.create_preview_link_inner(args).await;
        self.record_outcome("create_preview_link", &slug, &result);
        result
    }

    async fn create_preview_link_inner(
        &self,
        args: SlugOnly,
    ) -> Result<Json<PreviewView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        if project.status != ProjectStatus::Ready {
            return Err(not_running_error(&args.slug));
        }
        let port = project.port.ok_or_else(|| not_running_error(&args.slug))?;
        let token = self
            .authenticator
            .resolve_push_token(&project.repo_url)
            .await
            .map_err(into_mcp)?
            .ok_or_else(no_github_auth)?;
        let url = self
            .tunnel
            .share(
                &project.id,
                &project.name,
                port,
                &token,
                self.store.as_ref(),
            )
            .await
            .map_err(into_mcp)?;
        let live = self.tunnel.is_tunnel_live(&project.id).await;
        Ok(Json(PreviewView { url, live }))
    }

    /// Get the current preview-link URL if one is active. Returns `null`
    /// when no tunnel is registered for the project so the agent can
    /// distinguish "not shared" from "shared but offline".
    #[tool(
        description = "Get the current preview-link URL if one is active.",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    pub async fn get_preview_link(
        &self,
        Parameters(args): Parameters<SlugOnly>,
    ) -> Result<Json<PreviewLinkView>, McpError> {
        let slug = args.slug.clone();
        let result = self.get_preview_link_inner(args).await;
        self.record_outcome("get_preview_link", &slug, &result);
        result
    }

    async fn get_preview_link_inner(
        &self,
        args: SlugOnly,
    ) -> Result<Json<PreviewLinkView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        let preview = build_preview_view(&self.tunnel, &project.id).await;
        Ok(Json(PreviewLinkView { preview }))
    }

    /// Stop the preview link for a project. Idempotent: if no tunnel is
    /// active, [`TunnelCoordinator::unshare`] is a no-op.
    ///
    /// The github token is best-effort: cleanup must still succeed when the
    /// user has logged out, so a missing or failed token resolution falls
    /// back to an empty string. This mirrors `unshare_project`'s Tauri
    /// command, which deliberately swallows `AppError::AuthRequired` so the
    /// cloudflared process is always killed even when the API delete cannot
    /// authenticate.
    #[tool(
        description = "Stop the preview link for a project. Idempotent.",
        annotations(destructive_hint = true, idempotent_hint = true)
    )]
    pub async fn stop_preview_link(
        &self,
        Parameters(args): Parameters<SlugOnly>,
    ) -> Result<Json<StoppedView>, McpError> {
        let slug = args.slug.clone();
        let result = self.stop_preview_link_inner(args).await;
        self.record_outcome("stop_preview_link", &slug, &result);
        result
    }

    async fn stop_preview_link_inner(&self, args: SlugOnly) -> Result<Json<StoppedView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        let token = self
            .authenticator
            .resolve_push_token(&project.repo_url)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        self.tunnel
            .unshare(&project.id, &token, self.store.as_ref())
            .await
            .map_err(into_mcp)?;
        Ok(Json(StoppedView { stopped: true }))
    }
    // === Phase 6: github push + pages ===

    /// Commit the project's working tree (when dirty), make sure an `origin`
    /// remote exists (creating a GitHub repo when asked), then push the
    /// current branch.
    ///
    /// The flow is intentionally local-first: we attempt the commit + remote
    /// configuration before talking to GitHub at all, so a misconfigured
    /// worktree fails fast with a structured error instead of half-creating
    /// a remote repo.
    #[tool(
        description = "Commit and push a project's working tree to its GitHub remote. If no remote exists, optionally create the repo on GitHub first.",
        annotations(destructive_hint = true)
    )]
    pub async fn github_push(
        &self,
        Parameters(args): Parameters<GithubPushArgs>,
    ) -> Result<Json<PushView>, McpError> {
        let slug = args.slug.clone();
        let result = self.github_push_inner(args).await;
        self.record_outcome("github_push", &slug, &result);
        result
    }

    async fn github_push_inner(&self, args: GithubPushArgs) -> Result<Json<PushView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        let repo_path = project.repo_path();
        let token = self
            .authenticator
            .valid_token("github")
            .await
            .map_err(into_mcp)?
            .ok_or_else(|| {
                into_mcp(crate::error::AppError::AuthRequired {
                    provider: "github".into(),
                })
            })?;

        // 1. Validate remote state up front so a `create_repo_if_missing:
        //    false` call against an unconfigured project fails fast,
        //    BEFORE we mutate the working tree with commit_auto. Otherwise
        //    a misconfigured push would leave behind a stray commit.
        let mut created_repo = false;
        if !self
            .git
            .has_remote(&repo_path, "origin")
            .map_err(into_mcp)?
        {
            if !args.create_repo_if_missing {
                return Err(no_remote(&args.slug));
            }
            let repo_name = repo_name_for_push(&project, &args.slug)?;
            let private = matches!(args.visibility, PushVisibility::Private);
            let create_outcome = self
                .repos
                .create_user_repo(&token, &repo_name, private)
                .await;
            // Recovery path for partial-create failure: if a previous attempt
            // created the GitHub repo but didn't finish add_remote (libgit2
            // lock, transient I/O, etc.), the retry returns AlreadyExists.
            // Rather than wedging the agent in a 422 retry loop, fetch the
            // existing repo's clone URL and complete the local wiring.
            let clone_url = match create_outcome {
                Ok(created) => {
                    created_repo = true;
                    created.clone_url
                }
                Err(crate::error::AppError::AlreadyExists { entity, id })
                    if entity == "github_repo" =>
                {
                    let user = self
                        .authenticator
                        .github_user(&token)
                        .await
                        .map_err(into_mcp)?;
                    let existing = self
                        .repos
                        .fetch_user_repo(&token, &user.login, &repo_name)
                        .await
                        .map_err(into_mcp)?
                        .ok_or_else(|| {
                            into_mcp(crate::error::AppError::Internal {
                                reason: format!(
                                    "github reported repo `{id}` exists but it is not visible \
                                     to the authenticated user `{}`; check token scope",
                                    user.login
                                ),
                            })
                        })?;
                    // created_repo stays false: we adopted an existing one.
                    existing.clone_url
                }
                Err(other) => return Err(into_mcp(other)),
            };
            self.git
                .add_remote(&repo_path, "origin", &clone_url)
                .map_err(into_mcp)?;
        }

        // 2. Now that the remote story is settled, fold any dirty working
        //    tree into a single commit. `commit_auto` returns `NoOp` if
        //    the index ends up unchanged after staging (that's fine, we
        //    still push).
        let status = self.git.status(&repo_path).map_err(into_mcp)?;
        if status.dirty {
            let _ = self
                .git
                .commit_auto(&repo_path, &args.message)
                .map_err(into_mcp)?;
        }

        // 3. Push the current branch. `Failed` from libgit2 is wrapped in
        //    `Ok(...)` (local-first semantics), so we have to match instead
        //    of `?`-ing the result.
        let push_outcome = self
            .git
            .push_current_branch(&repo_path, Some(&token))
            .map_err(into_mcp)?;
        match push_outcome {
            PushOutcome::Pushed | PushOutcome::NothingToPush => {}
            PushOutcome::Failed(reason) => return Err(push_failed(&args.slug, &reason)),
        }

        // 4. Read back what just landed: HEAD sha for the agent's audit
        //    trail, remote url for the response.
        let commit_sha = self.git.current_commit_sha(&repo_path).map_err(into_mcp)?;
        let remote_url = self
            .git
            .remote_url(&repo_path, "origin")
            .map_err(into_mcp)?
            .ok_or_else(|| {
                into_mcp(crate::error::AppError::Internal {
                    reason: "origin remote has no decodable URL after push".into(),
                })
            })?;

        Ok(Json(PushView {
            remote_url,
            commit_sha,
            created_repo,
        }))
    }

    /// Build the project, force-push the artifacts to `gh-pages`, and
    /// enable GitHub Pages. Thin adapter over `publish::deployer::publish`:
    /// we only add the `tokio::time::timeout` ceiling and the
    /// agent-facing view shape.
    #[tool(
        description = "Build the project and publish it to GitHub Pages via the gh-pages branch. Blocks until the build completes (up to 10 minutes).",
        annotations(destructive_hint = true)
    )]
    pub async fn github_pages_publish(
        &self,
        Parameters(args): Parameters<SlugOnly>,
    ) -> Result<Json<PublishedView>, McpError> {
        let slug = args.slug.clone();
        let result = self.github_pages_publish_inner(args).await;
        self.record_outcome("github_pages_publish", &slug, &result);
        result
    }

    async fn github_pages_publish_inner(
        &self,
        args: SlugOnly,
    ) -> Result<Json<PublishedView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        let deps = PublishDeps {
            store: self.store.as_ref(),
            runtime: self.runtime.as_ref(),
            git: self.git.as_ref(),
            authenticator: &self.authenticator,
            pages_client: &self.pages,
        };
        let result =
            tokio::time::timeout(PUBLISH_DEADLINE, deployer::publish(&project.id, &deps)).await;

        let (pages_url, framework) = match result {
            Ok(r) => r.map_err(into_mcp)?,
            Err(_elapsed) => {
                return Err(publish_timeout(&args.slug, PUBLISH_DEADLINE.as_secs()));
            }
        };

        Ok(Json(PublishedView {
            url: pages_url,
            framework: framework_label(&framework).to_string(),
            deployed_at: unix_now_millis(),
        }))
    }

    /// Disable GitHub Pages for the project and remove the `gh-pages` branch
    /// on the remote. Idempotent: calling on an unpublished project is a
    /// no-op (the deployer's best-effort cleanup tolerates missing branch /
    /// disabled Pages).
    #[tool(
        description = "Disable GitHub Pages for a project and remove the gh-pages branch.",
        annotations(destructive_hint = true, idempotent_hint = true)
    )]
    pub async fn github_pages_unpublish(
        &self,
        Parameters(args): Parameters<SlugOnly>,
    ) -> Result<Json<UnpublishedView>, McpError> {
        let slug = args.slug.clone();
        let result = self.github_pages_unpublish_inner(args).await;
        self.record_outcome("github_pages_unpublish", &slug, &result);
        result
    }

    async fn github_pages_unpublish_inner(
        &self,
        args: SlugOnly,
    ) -> Result<Json<UnpublishedView>, McpError> {
        let project = project_for_slug(self.store.as_ref(), &args.slug).map_err(into_mcp)?;
        let deps = UnpublishDeps {
            store: self.store.as_ref(),
            authenticator: &self.authenticator,
            pages_client: &self.pages,
        };
        deployer::unpublish(&project.id, &deps)
            .await
            .map_err(into_mcp)?;
        Ok(Json(UnpublishedView { unpublished: true }))
    }

    #[tool(
        description = "Read a file from the project's working tree. Returns UTF-8 \
            content if the file decodes cleanly, base64 otherwise. Caps at max_bytes \
            (default 1 MiB, ceiling 4 MiB).",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    pub async fn read_file(
        &self,
        Parameters(args): Parameters<ReadFileArgs>,
    ) -> Result<Json<ReadFileView>, McpError> {
        let slug = args.slug.clone();
        let result = read_file_impl(self.store.as_ref(), args).await.map(Json);
        self.record_outcome("read_file", &slug, &result);
        result
    }

    #[tool(
        description = "Write content to a file in the project's working tree. \
            Encoding defaults to UTF-8; pass encoding='base64' to write binary. \
            Atomic via tempfile + rename. Caps at 4 MiB. Refuses to overwrite \
            an existing directory.",
        annotations(destructive_hint = true, idempotent_hint = true)
    )]
    pub async fn write_file(
        &self,
        Parameters(args): Parameters<WriteFileArgs>,
    ) -> Result<Json<WriteFileView>, McpError> {
        let slug = args.slug.clone();
        let result = write_file_impl(self.store.as_ref(), args).await.map(Json);
        self.record_outcome("write_file", &slug, &result);
        result
    }

    #[tool(
        description = "Execute a shell command inside the project's container. \
            Combined stdout+stderr capped at 256 KiB (128 KiB each). Default timeout \
            60s, ceiling 300s. Returns once command exits or the timeout fires.",
        annotations(destructive_hint = true)
    )]
    pub async fn exec_command(
        &self,
        Parameters(args): Parameters<ExecCommandArgs>,
    ) -> Result<Json<ExecCommandView>, McpError> {
        let slug = args.slug.clone();
        let result = exec_command_impl(self.store.as_ref(), self.exec.as_ref(), args)
            .await
            .map(Json);
        self.record_outcome("exec_command", &slug, &result);
        result
    }

    #[tool(
        description = "Snapshot the project's working tree via APFS CoW clone. \
            Returns instantly on macOS APFS; takes seconds on fallback copy. \
            Does NOT capture in-container process state or non-bind-mounted data dirs.",
        annotations(destructive_hint = true)
    )]
    pub async fn snapshot_project(
        &self,
        Parameters(args): Parameters<SnapshotArgs>,
    ) -> Result<Json<SnapshotView>, McpError> {
        let snapshot_root = match crate::constants::paths::snapshots_dir().map_err(into_mcp) {
            Ok(p) => p,
            Err(err) => {
                let result: Result<Json<SnapshotView>, McpError> = Err(err);
                self.record_outcome("snapshot_project", &args.slug, &result);
                return result;
            }
        };
        let slug = args.slug.clone();
        let result = snapshot_project_impl(self.store.as_ref(), args, &snapshot_root)
            .await
            .map(Json);
        self.record_outcome("snapshot_project", &slug, &result);
        result
    }

    #[tool(
        description = "Restore the project tree to a prior snapshot. Stops the dev \
            server, swaps the tree, restarts. Interruption typically 5-15s. \
            Does NOT restore in-container process state or non-bind-mounted data dirs.",
        annotations(destructive_hint = true, idempotent_hint = true)
    )]
    pub async fn rollback_project(
        &self,
        Parameters(args): Parameters<RollbackArgs>,
    ) -> Result<Json<RollbackView>, McpError> {
        let snapshot_root = match crate::constants::paths::snapshots_dir().map_err(into_mcp) {
            Ok(p) => p,
            Err(err) => {
                let result: Result<Json<RollbackView>, McpError> = Err(err);
                self.record_outcome("rollback_project", &args.slug, &result);
                return result;
            }
        };
        let slug = args.slug.clone();
        let result = rollback_project_impl(
            self.store.as_ref(),
            self.orch.as_ref(),
            Some(&self.app_handle),
            args,
            &snapshot_root,
        )
        .await
        .map(Json);
        self.record_outcome("rollback_project", &slug, &result);
        result
    }

    #[tool(
        description = "Fork a project: APFS-clone its tree to a new sandbox, start a \
            container, return the child slug. Parent must be Ready. Each child is a \
            full project with parent_project_id pointing back to the parent.",
        annotations(destructive_hint = true)
    )]
    pub async fn fork_project(
        &self,
        Parameters(args): Parameters<ForkArgs>,
    ) -> Result<Json<ForkChildrenView>, McpError> {
        let repos_root = match crate::constants::paths::repos_dir().map_err(into_mcp) {
            Ok(p) => p,
            Err(err) => {
                let result: Result<Json<ForkChildrenView>, McpError> = Err(err);
                self.record_outcome("fork_project", &args.slug, &result);
                return result;
            }
        };
        let slug = args.slug.clone();
        let result = fork_project_impl(
            self.store.as_ref(),
            self.orch.as_ref(),
            Some(&self.app_handle),
            args,
            &repos_root,
        )
        .await
        .map(|children| Json(ForkChildrenView { children }));
        self.record_outcome("fork_project", &slug, &result);
        result
    }
}

/// Free-function body of the `read_file` MCP tool, factored out so unit tests
/// can exercise it against a stub `ProjectStore` without building a full
/// `OpnbleMcp` (which would need a real `tauri::AppHandle`, orchestrator,
/// runtime, and tunnel coordinator the read path never touches).
async fn read_file_impl(
    store: &dyn crate::projects::store::ProjectStore,
    args: ReadFileArgs,
) -> Result<ReadFileView, McpError> {
    use base64::Engine;
    use tokio::io::AsyncReadExt;
    const MAX_READ_CEILING: u64 = 4 * 1024 * 1024;
    let max_bytes = args.max_bytes.min(MAX_READ_CEILING);
    let project = project_for_slug(store, &args.slug).map_err(into_mcp)?;
    let abs = crate::mcp::path::safe_join_within_repo(&project.repo_path(), &args.path, true)
        .map_err(into_mcp)?;
    let metadata = tokio::fs::metadata(&abs).await.map_err(|e| {
        into_mcp(crate::error::AppError::InvalidInput {
            field: "path".into(),
            reason: format!("stat: {e}"),
        })
    })?;
    let size = metadata.len();
    let read_n = size.min(max_bytes);
    let mut buf = vec![0u8; usize::try_from(read_n).unwrap_or(usize::MAX)];
    let mut file = tokio::fs::File::open(&abs).await.map_err(|e| {
        into_mcp(crate::error::AppError::Internal {
            reason: format!("open: {e}"),
        })
    })?;
    let n = file.read(&mut buf).await.map_err(|e| {
        into_mcp(crate::error::AppError::Internal {
            reason: format!("read: {e}"),
        })
    })?;
    buf.truncate(n);
    let (content, encoding) = match std::str::from_utf8(&buf) {
        Ok(s) => (s.to_string(), "utf-8".to_string()),
        Err(_) => (
            base64::engine::general_purpose::STANDARD.encode(&buf),
            "base64".to_string(),
        ),
    };
    Ok(ReadFileView {
        content,
        encoding,
        size,
        truncated: size > max_bytes,
    })
}

/// Free-function body of the `write_file` MCP tool. Mirrors `read_file_impl`'s
/// test seam: takes a `&dyn ProjectStore` so unit tests can drive it against an
/// in-memory store without the rest of `OpnbleMcp`'s deps.
///
/// Writes are atomic via tempfile + rename, capped at 4 MiB, and refuse to
/// overwrite a path whose leaf component resolves to an existing directory
/// (see Task 3.1 review: `safe_join_within_repo(must_exist=false)` can return
/// a path pointing at an existing directory when the leaf collides).
async fn write_file_impl(
    store: &dyn crate::projects::store::ProjectStore,
    args: WriteFileArgs,
) -> Result<WriteFileView, McpError> {
    use base64::Engine;
    const MAX_WRITE_BYTES: usize = 4 * 1024 * 1024;

    let bytes: Vec<u8> = match args.encoding.as_deref() {
        Some("base64") => base64::engine::general_purpose::STANDARD
            .decode(&args.content)
            .map_err(|e| {
                into_mcp(crate::error::AppError::InvalidInput {
                    field: "content".into(),
                    reason: format!("invalid base64: {e}"),
                })
            })?,
        None | Some("utf-8") => args.content.into_bytes(),
        Some(other) => {
            return Err(into_mcp(crate::error::AppError::InvalidInput {
                field: "encoding".into(),
                reason: format!("unsupported encoding: {other}"),
            }));
        }
    };

    if bytes.len() > MAX_WRITE_BYTES {
        return Err(into_mcp(crate::error::AppError::InvalidInput {
            field: "content".into(),
            reason: format!("content exceeds {MAX_WRITE_BYTES} bytes"),
        }));
    }

    let project = project_for_slug(store, &args.slug).map_err(into_mcp)?;
    let abs = crate::mcp::path::safe_join_within_repo(&project.repo_path(), &args.path, false)
        .map_err(into_mcp)?;

    // Defend against the leaf-is-a-directory case (see Task 3.1 review).
    // safe_join_within_repo with must_exist=false returns canonical_parent.join(file_name),
    // which can point at an existing directory if the requested path's leaf collides
    // with one. Refuse to clobber directories.
    if abs.is_dir() {
        return Err(into_mcp(crate::error::AppError::InvalidInput {
            field: "path".into(),
            reason: "target is an existing directory".into(),
        }));
    }

    let tmp = abs.with_extension("opnble.tmp");
    {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::File::create(&tmp).await.map_err(|e| {
            into_mcp(crate::error::AppError::Internal {
                reason: format!("cannot create tempfile: {e}"),
            })
        })?;
        file.write_all(&bytes).await.map_err(|e| {
            into_mcp(crate::error::AppError::Internal {
                reason: format!("cannot write: {e}"),
            })
        })?;
        file.sync_all().await.map_err(|e| {
            into_mcp(crate::error::AppError::Internal {
                reason: format!("cannot sync: {e}"),
            })
        })?;
    }
    tokio::fs::rename(&tmp, &abs).await.map_err(|e| {
        into_mcp(crate::error::AppError::Internal {
            reason: format!("cannot rename: {e}"),
        })
    })?;

    Ok(WriteFileView {
        bytes_written: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    })
}

/// Free-function body of the `exec_command` MCP tool. Takes the `ProjectStore`
/// and `ExecRunner` as trait objects so unit tests can drive it with an
/// in-memory store and a stub runner — neither a live VM nor a real
/// `OpnbleMcp` is required.
async fn exec_command_impl(
    store: &dyn crate::projects::store::ProjectStore,
    exec: &dyn crate::mcp::exec::ExecRunner,
    args: ExecCommandArgs,
) -> Result<ExecCommandView, McpError> {
    const MAX_STREAM_BYTES: usize = 131_072; // 128 KiB per stream, 256 KiB total
    const MAX_TIMEOUT_MS: u64 = 300_000;

    let timeout_ms = args.timeout_ms.min(MAX_TIMEOUT_MS);
    let timeout_secs = timeout_ms.div_ceil(1_000);

    let project = project_for_slug(store, &args.slug).map_err(into_mcp)?;
    if project.status != ProjectStatus::Ready {
        return Err(not_running_error(&args.slug));
    }

    // Validate env keys: no whitespace, no `=`, non-empty. The wrapper passes
    // each pair as `-e K=V`, so a `=` or whitespace in K would silently shift
    // the boundary between key and value (or split the flag entirely).
    let env: Vec<(String, String)> = args.env.into_iter().collect();
    for (k, _) in &env {
        if k.is_empty() || k.contains('=') || k.chars().any(char::is_whitespace) {
            return Err(into_mcp(crate::error::AppError::InvalidInput {
                field: "env".into(),
                reason: format!("invalid env name: {k:?}"),
            }));
        }
    }

    let container = project.id.container_name();
    let wrapper = build_exec_wrapper(
        &container,
        &args.cmd,
        args.cwd.as_deref(),
        &env,
        timeout_secs,
    );

    let started = std::time::Instant::now();
    let (exit_code, mut stdout, mut stderr) = exec.run(&wrapper).await.map_err(into_mcp)?;

    let truncated_out = stdout.len() > MAX_STREAM_BYTES;
    if truncated_out {
        stdout.truncate(MAX_STREAM_BYTES);
    }
    let truncated_err = stderr.len() > MAX_STREAM_BYTES;
    if truncated_err {
        stderr.truncate(MAX_STREAM_BYTES);
    }

    // `timeout(1)` exits 124 when the wrapped command exceeds the deadline.
    // Surface this as a structured error so agents can react (retry with a
    // larger budget, split the command) instead of guessing why exit_code=124.
    if exit_code == 124 {
        return Err(exec_timeout_error(&args.slug, timeout_ms));
    }

    Ok(ExecCommandView {
        stdout,
        stderr,
        exit_code,
        truncated: truncated_out || truncated_err,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

/// Free-function body of the `snapshot_project` MCP tool. Takes the
/// `ProjectStore` and snapshot root directory so unit tests can drive it
/// against a temp dir without a real `OpnbleMcp`.
///
/// On macOS APFS, `clone_tree_or_copy` is near-instant via `clonefile(2)`;
/// elsewhere it recursively copies. The snapshot captures the working tree
/// only — in-container process state and bind-mount-external data are not
/// included (caller's responsibility to document for agents).
#[allow(
    clippy::unused_async,
    reason = "MCP tool wrapper awaits this; the body is sync today but the contract stays async"
)]
async fn snapshot_project_impl(
    store: &dyn crate::projects::store::ProjectStore,
    args: SnapshotArgs,
    snapshot_root: &Path,
) -> Result<SnapshotView, McpError> {
    let project = project_for_slug(store, &args.slug).map_err(into_mcp)?;
    let snapshot_id = crate::snapshots::SnapshotId::generate();
    let dest = snapshot_root
        .join(project.id.as_str())
        .join(snapshot_id.as_str());
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            into_mcp(crate::error::AppError::StorageFailed {
                reason: format!("mkdir snapshot parent: {e}"),
            })
        })?;
    }
    crate::vm::clone::clone_tree_or_copy(&project.repo_path(), &dest).map_err(|e| {
        into_mcp(crate::error::AppError::StorageFailed {
            reason: format!("clone tree: {e}"),
        })
    })?;
    let taken_at = unix_now_millis();
    let record = crate::snapshots::SnapshotRecord {
        id: snapshot_id.clone(),
        project_id: project.id.clone(),
        label: args.label.clone(),
        taken_at,
        size_bytes: 0,
    };
    store.add_snapshot(record).map_err(into_mcp)?;
    Ok(SnapshotView {
        snapshot_id: snapshot_id.as_str().to_string(),
        label: args.label,
        taken_at,
    })
}

/// Free-function body of the `rollback_project` MCP tool. Takes
/// `Option<&tauri::AppHandle>` so unit tests can pass `None` for projects
/// whose `port` is `None` (no restart needed). The production wrapper always
/// passes `Some(&self.app_handle)`.
///
/// Sequence: stop the container, swap the tree (`rm -rf` + clone), restart if
/// the project had a port. Snapshot IDs from other projects are refused so a
/// confused caller can't restore project A's tree onto project B.
pub(crate) async fn rollback_project_impl(
    store: &dyn crate::projects::store::ProjectStore,
    orch: &dyn crate::mcp::orch::OrchestratorOps,
    app: Option<&tauri::AppHandle>,
    args: RollbackArgs,
    snapshot_root: &Path,
) -> Result<RollbackView, McpError> {
    let project = project_for_slug(store, &args.slug).map_err(into_mcp)?;
    let snapshot_id =
        crate::snapshots::SnapshotId::new(args.snapshot_id.clone()).map_err(|_| {
            into_mcp(crate::error::AppError::InvalidInput {
                field: "snapshot_id".into(),
                reason: "not a valid snapshot id".into(),
            })
        })?;
    let record = store.get_snapshot(&snapshot_id).map_err(into_mcp)?;
    if record.project_id != project.id {
        return Err(into_mcp(crate::error::AppError::InvalidInput {
            field: "snapshot_id".into(),
            reason: "snapshot does not belong to this project".into(),
        }));
    }
    let snap_path = snapshot_root
        .join(project.id.as_str())
        .join(snapshot_id.as_str());
    if !snap_path.exists() {
        return Err(into_mcp(crate::error::AppError::StorageFailed {
            reason: format!("snapshot dir missing: {}", snap_path.display()),
        }));
    }

    let started = std::time::Instant::now();
    let port = project.port.unwrap_or(0);

    orch.force_stop_and_remove(project.id.as_str())
        .await
        .map_err(into_mcp)?;

    if project.repo_path().exists() {
        std::fs::remove_dir_all(project.repo_path()).map_err(|e| {
            into_mcp(crate::error::AppError::StorageFailed {
                reason: format!("rm repo: {e}"),
            })
        })?;
    }
    crate::vm::clone::clone_tree_or_copy(&snap_path, &project.repo_path()).map_err(|e| {
        into_mcp(crate::error::AppError::StorageFailed {
            reason: format!("clone snap: {e}"),
        })
    })?;

    if port > 0 {
        if let Some(app_handle) = app {
            orch.start_project(app_handle, project.id.as_str(), port)
                .await
                .map_err(into_mcp)?;
            let _ = crate::mcp::wait::wait_for_status(
                app_handle,
                store,
                &project.id,
                ProjectStatus::Ready,
                std::time::Duration::from_mins(2),
            )
            .await;
        }
    }

    let interruption_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    Ok(RollbackView {
        snapshot_id: snapshot_id.as_str().to_string(),
        restored_at: unix_now_millis(),
        interruption_ms,
    })
}

/// Free-function body of the `fork_project` MCP tool. Clones the parent's
/// working tree to a fresh sandbox under `repos_root`, rewrites the
/// committed `.opnble` marker with a child slug derived from the parent's
/// slug plus a short suffix of the new project id, best-effort writes the
/// child's `.cursor/mcp.json` (failures logged), allocates a port,
/// registers the new project with `parent_project_id`/`forked_at`, and
/// starts the orchestrator when an `AppHandle` is supplied. Inherits
/// `installed` from the parent so the orchestrator skips the npm install
/// step on first boot.
async fn fork_project_impl(
    store: &dyn crate::projects::store::ProjectStore,
    orch: &dyn crate::mcp::orch::OrchestratorOps,
    app: Option<&tauri::AppHandle>,
    args: ForkArgs,
    repos_root: &Path,
) -> Result<Vec<ForkChild>, McpError> {
    const MAX_COUNT: u32 = 4;
    let count = args.count.clamp(1, MAX_COUNT);

    let parent = project_for_slug(store, &args.slug).map_err(into_mcp)?;
    if parent.status != ProjectStatus::Ready {
        return Err(not_running_error(&args.slug));
    }
    let parent_slug = parent.slug.clone().ok_or_else(|| {
        into_mcp(crate::error::AppError::InvalidInput {
            field: "slug".into(),
            reason: "parent project has no slug".into(),
        })
    })?;

    let mut children = Vec::with_capacity(usize::try_from(count).unwrap_or(0));

    for _ in 0..count {
        let new_id = ProjectId::generate();
        let new_repo = repos_root.join(new_id.as_str());

        // Clone the parent's tree to the child's path (APFS CoW on macOS,
        // recursive copy fallback elsewhere).
        crate::vm::clone::clone_tree_or_copy(&parent.repo_path(), &new_repo).map_err(|e| {
            into_mcp(crate::error::AppError::StorageFailed {
                reason: format!("fork clone: {e}"),
            })
        })?;

        // Derive the child slug from the parent slug + a 6-char suffix from
        // the new id. Suffix is taken from the tail of the timestamp-based
        // id so duplicate suffixes within a single millisecond are
        // practically impossible.
        let id_str = new_id.as_str();
        let suffix: String = id_str
            .chars()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let child_slug = format!("{parent_slug}-{suffix}");

        // Rewrite the .opnble marker with the child slug so cross-machine
        // resolution finds the fork independently of the parent.
        let marker =
            crate::projects::marker::Marker::new(child_slug.clone(), parent.repo_url.clone());
        crate::projects::marker::write_marker(&new_repo, &marker).map_err(into_mcp)?;

        // Best-effort .cursor/mcp.json install. Failures log and continue
        // so a missing token does not abort the fork.
        if let Ok(token) = crate::mcp::auth::current_token() {
            if !token.is_empty() {
                if let Err(e) = crate::mcp::install::write_project_mcp_config(
                    &new_repo,
                    &token,
                    crate::mcp::config::PORT,
                ) {
                    tracing::warn!("fork: cannot install .cursor/mcp.json for {child_slug}: {e}");
                }
                if let Err(e) = crate::mcp::install::append_to_gitignore(
                    &new_repo,
                    crate::mcp::install::MCP_CONFIG_REL,
                ) {
                    tracing::warn!("fork: cannot patch .gitignore for {child_slug}: {e}");
                }
            }
        }

        let port = store.allocate_port().map_err(into_mcp)?;

        // Build the child Project. Inherit `installed` from the parent so
        // the orchestrator's first-boot install step is skipped: the cloned
        // tree already carries the parent's `node_modules`.
        let child = Project::new(
            new_id.clone(),
            format!("{} (fork)", parent.name),
            parent.repo_url.clone(),
            new_repo.to_string_lossy().into_owned(),
            parent.branch.clone(),
        )
        .with_slug(child_slug.clone())
        .with_parent(parent.id.clone(), unix_now_millis());
        let child = Project {
            installed: parent.installed,
            ..child
        };

        store.add(child.clone()).map_err(into_mcp)?;

        // Start the container only if we have an AppHandle. Tests pass
        // `None` to exercise everything up to the orchestrator call.
        if let Some(app_handle) = app {
            orch.start_project(app_handle, new_id.as_str(), port)
                .await
                .map_err(into_mcp)?;
            let _ = wait_for_status(
                app_handle,
                store,
                &new_id,
                ProjectStatus::Ready,
                std::time::Duration::from_mins(5),
            )
            .await;
        }

        // Refresh status from the store; falls back to the local `child`
        // value when the store doesn't expose the updated row.
        let final_project = store.get(&new_id).unwrap_or(child);
        children.push(ForkChild {
            slug: child_slug,
            parent_slug: parent_slug.clone(),
            status: final_project.status.to_string(),
            port: final_project.port,
            url: final_project.port.map(|p| format!("http://127.0.0.1:{p}/")),
        });
    }

    Ok(children)
}

/// Filter helper for `list_projects`: drop projects without a portable slug
/// since the agent has no addressable identifier for them.
fn project_view_if_addressable(project: &Project) -> Option<ProjectView> {
    project
        .slug
        .as_deref()
        .map(|slug| ProjectView::from_project_with_slug(project, slug))
}

/// Construct a view for a project that just came back from `get_by_slug`.
/// `get_by_slug` only matches projects whose `slug` is `Some`, so falling
/// back to the lookup slug instead of `expect`-ing is safe and avoids a
/// panic if a future store implementation breaks the invariant.
fn project_view_for_resolved(project: &Project, lookup_slug: &str) -> ProjectView {
    let slug = project.slug.as_deref().unwrap_or(lookup_slug);
    ProjectView::from_project_with_slug(project, slug)
}

/// Idempotent shortcut for `start_project`: if the project is already
/// `Ready` with a port, build the running view directly without flapping
/// status through `Starting`.
fn idempotent_running_view(project: &Project) -> Option<RunningView> {
    if project.status == ProjectStatus::Ready {
        if let Some(port) = project.port {
            return Some(RunningView::for_port(port));
        }
    }
    None
}

/// Clamp the caller-supplied `lines` argument into the
/// [`DEFAULT_TAIL_LINES`]..=[`MAX_TAIL_LINES`] band. Extracted so the cap
/// math has a unit test of its own without spinning up an `OpnbleMcp`.
fn cap_lines(arg: Option<u32>) -> u32 {
    arg.unwrap_or(DEFAULT_TAIL_LINES).min(MAX_TAIL_LINES)
}

fn not_installed_error(slug: &str) -> McpError {
    McpError {
        code: INVALID_PARAMS,
        message: format!("cannot start '{slug}': project is not installed").into(),
        data: Some(json!({
            "code": "NOT_INSTALLED",
            "hint": "run install_project first or open the project in Opnble to install",
        })),
    }
}

fn start_failed_error(slug: &str, summary: Option<String>) -> McpError {
    let detail = summary.unwrap_or_else(|| "project transitioned to failed".to_string());
    McpError {
        code: INTERNAL_ERROR,
        message: format!("cannot start '{slug}': {detail}").into(),
        data: Some(json!({ "code": "START_FAILED", "hint": detail })),
    }
}

fn start_timeout_error(slug: &str, tail: Option<&str>) -> McpError {
    let secs = START_DEADLINE.as_secs();
    let hint = tail.map_or_else(
        || "no recent container logs available".to_string(),
        str::to_string,
    );
    McpError {
        code: INTERNAL_ERROR,
        message: format!("cannot start '{slug}': not ready after {secs}s").into(),
        data: Some(json!({ "code": "START_TIMEOUT", "hint": hint })),
    }
}

fn stop_failed_error(slug: &str, summary: Option<String>) -> McpError {
    let detail = summary.unwrap_or_else(|| "project transitioned to failed".to_string());
    McpError {
        code: INTERNAL_ERROR,
        message: format!("cannot stop '{slug}': {detail}").into(),
        data: Some(json!({ "code": "STOP_FAILED", "hint": detail })),
    }
}

fn stop_timeout_error(slug: &str) -> McpError {
    let secs = STOP_DEADLINE.as_secs();
    McpError {
        code: INTERNAL_ERROR,
        message: format!("cannot stop '{slug}': did not stop within {secs}s").into(),
        data: Some(json!({
            "code": "STOP_TIMEOUT",
            "hint": "force-restart the VM if the container is wedged",
        })),
    }
}

/// Phase 5 precondition error: tunnel sharing requires a `Ready` project
/// with an allocated port. Both the missing-port and wrong-status cases
/// collapse to the same `NOT_RUNNING` code because the agent's recovery is
/// identical: bring the dev server up first.
fn not_running_error(slug: &str) -> McpError {
    McpError {
        code: INVALID_PARAMS,
        message: format!("cannot share '{slug}': project is not running").into(),
        data: Some(json!({
            "code": "NOT_RUNNING",
            "hint": "Call start_project first to bring the dev server up",
        })),
    }
}

/// `timeout(1)` returns 124 when the wrapped command exceeds its budget. We
/// surface that as a structured `EXEC_TIMEOUT` instead of an opaque
/// `exit_code=124` so agents have an unambiguous signal to widen `timeout_ms`
/// or split the command.
fn exec_timeout_error(slug: &str, timeout_ms: u64) -> McpError {
    McpError {
        code: INVALID_PARAMS,
        message: format!("exec_command timed out after {timeout_ms}ms for '{slug}'").into(),
        data: Some(json!({
            "code": "EXEC_TIMEOUT",
            "hint": "increase timeout_ms or split into smaller commands",
        })),
    }
}

/// Build the option-shaped view consumed by `get_preview_link`. Extracted
/// from the tool body so it can be unit-tested against a real
/// [`TunnelCoordinator`] without standing up a Tauri `AppHandle`.
async fn build_preview_view(
    coordinator: &TunnelCoordinator,
    project_id: &ProjectId,
) -> Option<PreviewView> {
    let url = coordinator.tunnel_url(project_id).await?;
    let live = coordinator.is_tunnel_live(project_id).await;
    Some(PreviewView { url, live })
}

fn repo_name_for_push(project: &Project, lookup_slug: &str) -> Result<String, McpError> {
    if let Some(slug) = project.slug.as_deref() {
        if !slug.is_empty() {
            return Ok(slug.to_string());
        }
    }
    if !lookup_slug.is_empty() {
        return Ok(lookup_slug.to_string());
    }
    Err(into_mcp(crate::error::AppError::InvalidInput {
        field: "slug".into(),
        reason: "project has no slug; cannot derive a repo name".into(),
    }))
}

/// Build the shell string sent to `agent_client::exec_host`.
///
/// Layout: `timeout {secs}s podman exec [-w /app/{cwd}] [-e K=V ...] {container} sh -c {shlex-quoted-cmd}`.
/// `cmd` is shlex-quoted so semicolons, quotes, and shell metacharacters
/// inside it cannot break out of the `sh -c` boundary.
fn build_exec_wrapper(
    container: &str,
    cmd: &str,
    cwd_relative_to_app: Option<&str>,
    env: &[(String, String)],
    timeout_secs: u64,
) -> String {
    let mut parts: Vec<String> = vec![
        "timeout".into(),
        format!("{timeout_secs}s"),
        "podman".into(),
        "exec".into(),
    ];
    if let Some(cwd) = cwd_relative_to_app {
        parts.push("-w".into());
        parts.push(format!("/app/{}", cwd.trim_start_matches('/')));
    }
    for (k, v) in env {
        parts.push("-e".into());
        parts.push(format!("{k}={v}"));
    }
    parts.push(container.to_string());
    parts.push("sh".into());
    parts.push("-c".into());
    parts.push(shlex::try_quote(cmd).expect("shlex quote").into_owned());
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::types::{ProjectId, ProjectIntent};
    use serde_json::Value;

    /// rmcp 1.7.0 panics when building the router if any tool's output schema
    /// has a non-object root (e.g. a bare `Json<Vec<_>>` return). This guards
    /// against reintroducing array-rooted tool outputs.
    #[test]
    fn tool_router_builds_with_object_rooted_schemas() {
        let _router = crate::mcp::server::OpnbleMcp::tool_router();
    }

    fn make_project(
        id: &str,
        slug: Option<&str>,
        status: ProjectStatus,
        port: Option<u16>,
        installed: bool,
    ) -> Project {
        Project {
            id: ProjectId::new(id).expect("valid id"),
            name: "Test".to_string(),
            slug: slug.map(str::to_string),
            repo_url: "https://example.com/repo".to_string(),
            local_path: format!("/tmp/{id}"),
            status,
            intent: ProjectIntent::Run,
            port,
            branch: "main".to_string(),
            last_synced_at: 0,
            tunnel_id: None,
            tunnel_url: None,
            pages_url: None,
            installed,
            parent_project_id: None,
            forked_at: None,
        }
    }

    #[test]
    fn project_view_if_addressable_skips_projects_without_slug() {
        let with_slug = make_project("p1", Some("blog"), ProjectStatus::Stopped, None, true);
        let without = make_project("p2", None, ProjectStatus::Stopped, None, true);
        assert!(project_view_if_addressable(&with_slug).is_some());
        assert!(project_view_if_addressable(&without).is_none());
    }

    #[test]
    fn idempotent_running_view_only_fires_when_ready_with_port() {
        let ready = make_project("p1", Some("blog"), ProjectStatus::Ready, Some(3005), true);
        let starting = make_project(
            "p1",
            Some("blog"),
            ProjectStatus::Starting,
            Some(3005),
            true,
        );
        let ready_no_port = make_project("p1", Some("blog"), ProjectStatus::Ready, None, true);
        assert!(idempotent_running_view(&ready).is_some());
        assert!(idempotent_running_view(&starting).is_none());
        assert!(idempotent_running_view(&ready_no_port).is_none());
    }

    #[test]
    fn project_view_for_resolved_uses_project_slug_when_present() {
        let project = make_project("p1", Some("the-blog"), ProjectStatus::Stopped, None, true);
        let view = project_view_for_resolved(&project, "the-blog");
        assert_eq!(view.slug, "the-blog");
    }

    #[test]
    fn project_view_for_resolved_falls_back_to_lookup_slug() {
        let project = make_project("p1", None, ProjectStatus::Stopped, None, true);
        let view = project_view_for_resolved(&project, "the-blog");
        assert_eq!(view.slug, "the-blog");
    }

    fn data_code(err: &McpError) -> String {
        err.data
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|o| o.get("code"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    fn data_hint(err: &McpError) -> Option<String> {
        err.data
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|o| o.get("hint"))
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    #[test]
    fn not_installed_error_carries_actionable_hint() {
        let err = not_installed_error("blog");
        assert_eq!(err.code, INVALID_PARAMS);
        assert_eq!(data_code(&err), "NOT_INSTALLED");
        assert!(data_hint(&err).expect("hint").contains("install_project"));
        assert!(err.message.contains("blog"));
    }

    #[test]
    fn start_failed_error_uses_summary_when_available() {
        let err = start_failed_error("blog", Some("bind: address in use".into()));
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(data_code(&err), "START_FAILED");
        assert_eq!(data_hint(&err).as_deref(), Some("bind: address in use"));
    }

    #[test]
    fn start_failed_error_falls_back_when_summary_missing() {
        let err = start_failed_error("blog", None);
        assert_eq!(data_code(&err), "START_FAILED");
        assert!(data_hint(&err).expect("hint").contains("failed"));
    }

    #[test]
    fn start_timeout_error_uses_log_tail_when_available() {
        let err = start_timeout_error("blog", Some("npm ERR! exit 1"));
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(data_code(&err), "START_TIMEOUT");
        assert_eq!(data_hint(&err).as_deref(), Some("npm ERR! exit 1"));
        assert!(err.message.contains("300s"));
    }

    #[test]
    fn start_timeout_error_falls_back_when_no_logs() {
        let err = start_timeout_error("blog", None);
        assert!(data_hint(&err).expect("hint").contains("no recent"));
    }

    #[test]
    fn stop_failed_error_uses_summary_when_available() {
        let err = stop_failed_error("blog", Some("container exited with 137".into()));
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(data_code(&err), "STOP_FAILED");
        assert_eq!(
            data_hint(&err).as_deref(),
            Some("container exited with 137")
        );
    }

    #[test]
    fn stop_timeout_error_carries_recovery_hint() {
        let err = stop_timeout_error("blog");
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(data_code(&err), "STOP_TIMEOUT");
        assert!(err.message.contains("120s"));
        assert!(data_hint(&err).expect("hint").contains("VM"));
    }

    // === Phase 3: logs ===

    #[test]
    fn cap_lines_uses_default_when_none() {
        assert_eq!(cap_lines(None), DEFAULT_TAIL_LINES);
        assert_eq!(cap_lines(None), 200);
    }

    #[test]
    fn cap_lines_passes_small_values_through() {
        assert_eq!(cap_lines(Some(0)), 0);
        assert_eq!(cap_lines(Some(1)), 1);
        assert_eq!(cap_lines(Some(199)), 199);
        assert_eq!(cap_lines(Some(MAX_TAIL_LINES)), MAX_TAIL_LINES);
    }

    #[test]
    fn cap_lines_clamps_oversize_to_max() {
        assert_eq!(cap_lines(Some(10_000)), MAX_TAIL_LINES);
        assert_eq!(cap_lines(Some(MAX_TAIL_LINES + 1)), MAX_TAIL_LINES);
        assert_eq!(cap_lines(Some(u32::MAX)), MAX_TAIL_LINES);
    }

    #[test]
    fn tail_logs_view_serializes_camel_case_json() {
        let view = TailLogsView {
            lines: vec!["first".to_string(), "second".to_string()],
            truncated: true,
        };
        let json = serde_json::to_value(&view).expect("serialize");
        let obj = json.as_object().expect("object");
        assert!(obj.contains_key("lines"));
        assert!(obj.contains_key("truncated"));
        assert_eq!(obj.get("truncated").and_then(Value::as_bool), Some(true));
        assert_eq!(
            obj.get("lines")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            2
        );
    }

    #[test]
    fn tail_logs_view_empty_lines_serializes_as_empty_array() {
        let view = TailLogsView {
            lines: Vec::new(),
            truncated: false,
        };
        let json = serde_json::to_value(&view).expect("serialize");
        let lines = json.get("lines").and_then(Value::as_array).expect("array");
        assert!(lines.is_empty());
        assert_eq!(json.get("truncated").and_then(Value::as_bool), Some(false));
    }

    // === Phase 4: git view tests ===

    use crate::infrastructure::git::{FileChange, FileStatus, RepoStatus};

    fn sample_repo_status() -> RepoStatus {
        RepoStatus {
            branch: "main".to_string(),
            ahead: 2,
            behind: 1,
            dirty: true,
            files: vec![
                FileStatus {
                    path: "src/lib.rs".to_string(),
                    status: FileChange::Modified,
                },
                FileStatus {
                    path: "README.md".to_string(),
                    status: FileChange::Untracked,
                },
                FileStatus {
                    path: "old/path.rs".to_string(),
                    status: FileChange::Renamed,
                },
            ],
        }
    }

    #[test]
    fn git_status_view_round_trips_scalar_fields() {
        let status = sample_repo_status();
        let view = GitStatusView::from_repo_status(&status);

        assert_eq!(view.branch, "main");
        assert_eq!(view.ahead, 2);
        assert_eq!(view.behind, 1);
        assert!(view.dirty);
        assert_eq!(view.files.len(), 3);
    }

    #[test]
    fn git_status_view_maps_file_change_to_camel_case_string() {
        let status = sample_repo_status();
        let view = GitStatusView::from_repo_status(&status);

        let by_path: std::collections::HashMap<&str, &str> = view
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.status.as_str()))
            .collect();

        assert_eq!(by_path.get("src/lib.rs"), Some(&"modified"));
        assert_eq!(by_path.get("README.md"), Some(&"untracked"));
        assert_eq!(by_path.get("old/path.rs"), Some(&"renamed"));
    }

    #[test]
    fn git_status_view_clamps_huge_ahead_behind_to_u32_max() {
        let huge = RepoStatus {
            branch: "main".to_string(),
            ahead: usize::MAX,
            behind: usize::MAX,
            dirty: false,
            files: Vec::new(),
        };
        let view = GitStatusView::from_repo_status(&huge);
        assert_eq!(view.ahead, u32::MAX);
        assert_eq!(view.behind, u32::MAX);
    }

    #[test]
    fn git_status_view_serializes_camel_case_json() {
        let status = sample_repo_status();
        let view = GitStatusView::from_repo_status(&status);
        let json = serde_json::to_value(&view).expect("serialize");
        let obj = json.as_object().expect("object");
        assert!(obj.contains_key("branch"));
        assert!(obj.contains_key("ahead"));
        assert!(obj.contains_key("behind"));
        assert!(obj.contains_key("dirty"));
        assert!(obj.contains_key("files"));
        let files = obj.get("files").and_then(Value::as_array).expect("array");
        let first = files.first().and_then(Value::as_object).expect("entry");
        assert!(first.contains_key("path"));
        assert!(first.contains_key("status"));
    }

    #[test]
    fn file_status_view_uses_serde_compatible_string() {
        let file = FileStatus {
            path: "a.txt".to_string(),
            status: FileChange::Deleted,
        };
        let view = FileStatusView::from_file(&file);
        assert_eq!(view.path, "a.txt");
        assert_eq!(view.status, "deleted");

        let serde_str = serde_json::to_string(&file.status).expect("serialize");
        assert_eq!(serde_str.trim_matches('"'), view.status);
    }
    // === Phase 5: preview link ===

    #[test]
    fn not_running_error_uses_invalid_params_and_actionable_hint() {
        let err = not_running_error("blog");
        assert_eq!(err.code, INVALID_PARAMS);
        assert_eq!(data_code(&err), "NOT_RUNNING");
        assert!(err.message.contains("blog"));
        assert!(data_hint(&err).expect("hint").contains("start_project"));
    }

    #[test]
    fn preview_view_serializes_camel_case_with_both_fields() {
        let view = PreviewView {
            url: "https://abc.opnble.dev/".to_string(),
            live: true,
        };
        let json = serde_json::to_value(&view).expect("serialize");
        let obj = json.as_object().expect("object");
        assert_eq!(
            obj.get("url").and_then(Value::as_str),
            Some("https://abc.opnble.dev/")
        );
        assert_eq!(obj.get("live").and_then(Value::as_bool), Some(true));
        assert_eq!(obj.len(), 2);
    }

    #[test]
    fn stopped_view_serializes_camel_case_with_stopped_true() {
        let view = StoppedView { stopped: true };
        let json = serde_json::to_value(&view).expect("serialize");
        let obj = json.as_object().expect("object");
        assert_eq!(obj.get("stopped").and_then(Value::as_bool), Some(true));
        assert_eq!(obj.len(), 1);
    }

    /// Smoke test for the `get_preview_link` happy-miss path: with no active
    /// tunnel registered, `build_preview_view` resolves to `None`. Uses a
    /// real [`TunnelCoordinator`] because its empty-map probe path doesn't
    /// invoke cloudflared, only the in-process `Mutex<HashMap>`.
    #[tokio::test]
    async fn build_preview_view_returns_none_when_no_active_tunnel() {
        use std::path::PathBuf;
        let coordinator = TunnelCoordinator::new(PathBuf::from("/nonexistent-cloudflared"))
            .expect("coordinator construction does not depend on cloudflared existing");
        let id = ProjectId::new("proj-1").expect("valid id");
        let view = build_preview_view(&coordinator, &id).await;
        assert!(view.is_none());
    }

    // === Phase 3.3: read_file ===

    use crate::projects::store::ProjectStore;

    /// Single-project in-memory store. Implements only the methods
    /// `read_file_impl` reaches (`get_by_slug` via the default `list` impl);
    /// the rest are no-ops so the test seam stays narrow.
    ///
    /// Tracks snapshots in a `Mutex<Vec<_>>` so Task 5.2 tests can assert the
    /// store received the record, not just the on-disk tree. Uses
    /// `std::sync::Mutex` (not `tokio::sync::Mutex`) because the lock is only
    /// taken inside synchronous trait methods and never held across `.await`
    /// points.
    struct SingleProjectStore {
        project: Project,
        #[allow(
            clippy::disallowed_types,
            reason = "lock never held across await; sync trait method"
        )]
        snapshots: std::sync::Mutex<Vec<crate::snapshots::SnapshotRecord>>,
    }

    impl SingleProjectStore {
        fn new(project: Project) -> Self {
            #[allow(
                clippy::disallowed_types,
                reason = "lock never held across await; sync trait method"
            )]
            let snapshots = std::sync::Mutex::new(Vec::new());
            Self { project, snapshots }
        }

        fn with_project(project: Project) -> Self {
            Self::new(project)
        }
    }

    impl ProjectStore for SingleProjectStore {
        fn list(&self) -> Result<Vec<Project>, crate::error::AppError> {
            Ok(vec![self.project.clone()])
        }
        fn get(&self, id: &ProjectId) -> Result<Project, crate::error::AppError> {
            if &self.project.id == id {
                Ok(self.project.clone())
            } else {
                Err(crate::error::AppError::NotFound {
                    entity: "project".into(),
                    id: id.to_string(),
                })
            }
        }
        fn add(&self, _project: Project) -> Result<(), crate::error::AppError> {
            Ok(())
        }
        fn update(&self, _project: Project) -> Result<(), crate::error::AppError> {
            Ok(())
        }
        fn remove(&self, _id: &ProjectId) -> Result<(), crate::error::AppError> {
            Ok(())
        }
        fn allocate_port(&self) -> Result<u16, crate::error::AppError> {
            Ok(3001)
        }
        fn next_port(&self) -> Result<u16, crate::error::AppError> {
            Ok(3001)
        }
        fn add_snapshot(
            &self,
            record: crate::snapshots::SnapshotRecord,
        ) -> Result<(), crate::error::AppError> {
            self.snapshots.lock().expect("poisoned").push(record);
            Ok(())
        }
        fn list_snapshots(
            &self,
            project_id: &ProjectId,
        ) -> Result<Vec<crate::snapshots::SnapshotRecord>, crate::error::AppError> {
            Ok(self
                .snapshots
                .lock()
                .expect("poisoned")
                .iter()
                .filter(|r| &r.project_id == project_id)
                .cloned()
                .collect())
        }
        fn remove_snapshot(
            &self,
            id: &crate::snapshots::SnapshotId,
        ) -> Result<(), crate::error::AppError> {
            Err(crate::error::AppError::NotFound {
                entity: "snapshot".into(),
                id: id.as_str().to_string(),
            })
        }
        fn get_snapshot(
            &self,
            id: &crate::snapshots::SnapshotId,
        ) -> Result<crate::snapshots::SnapshotRecord, crate::error::AppError> {
            self.snapshots
                .lock()
                .expect("poisoned")
                .iter()
                .find(|r| &r.id == id)
                .cloned()
                .ok_or_else(|| crate::error::AppError::NotFound {
                    entity: "snapshot".into(),
                    id: id.as_str().to_string(),
                })
        }
    }

    fn project_at_path(slug: &str, repo: &std::path::Path) -> Project {
        let mut p = make_project(
            "proj-test",
            Some(slug),
            ProjectStatus::Ready,
            Some(3001),
            true,
        );
        p.local_path = repo.to_string_lossy().into_owned();
        p
    }

    #[tokio::test]
    async fn read_file_returns_utf8_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("hello.txt"), b"world").unwrap();
        let store = SingleProjectStore::new(project_at_path("blog", dir.path()));

        let args = ReadFileArgs {
            slug: "blog".into(),
            path: "hello.txt".into(),
            max_bytes: 1024,
        };
        let view = read_file_impl(&store, args).await.unwrap();
        assert_eq!(view.content, "world");
        assert_eq!(view.encoding, "utf-8");
        assert_eq!(view.size, 5);
        assert!(!view.truncated);
    }

    #[tokio::test]
    async fn read_file_returns_base64_for_binary_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("bin.dat"), [0xff, 0xfe, 0x00, 0x01]).unwrap();
        let store = SingleProjectStore::new(project_at_path("blog", dir.path()));

        let args = ReadFileArgs {
            slug: "blog".into(),
            path: "bin.dat".into(),
            max_bytes: 1024,
        };
        let view = read_file_impl(&store, args).await.unwrap();
        assert_eq!(view.encoding, "base64");
        assert_eq!(view.size, 4);
    }

    #[tokio::test]
    async fn read_file_truncated_when_larger_than_max_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let big: Vec<u8> = (0..2000_u32)
            .map(|i| u8::try_from(i % 256).unwrap())
            .collect();
        std::fs::write(dir.path().join("big.bin"), &big).unwrap();
        let store = SingleProjectStore::new(project_at_path("blog", dir.path()));

        let args = ReadFileArgs {
            slug: "blog".into(),
            path: "big.bin".into(),
            max_bytes: 1000,
        };
        let view = read_file_impl(&store, args).await.unwrap();
        assert!(view.truncated);
        assert_eq!(view.size, 2000);
    }

    // === Phase 3.4: write_file ===

    #[tokio::test]
    async fn write_file_creates_file_with_utf8_content() {
        let dir = tempfile::tempdir().unwrap();
        let store = SingleProjectStore::new(project_at_path("blog", dir.path()));

        let args = WriteFileArgs {
            slug: "blog".into(),
            path: "new.txt".into(),
            content: "hello".into(),
            encoding: None,
            create_parents: true,
        };
        let view = write_file_impl(&store, args).await.unwrap();
        assert_eq!(view.bytes_written, 5);
        let on_disk = std::fs::read(dir.path().join("new.txt")).unwrap();
        assert_eq!(on_disk, b"hello");
    }

    #[tokio::test]
    async fn write_file_decodes_base64_when_encoding_is_base64() {
        use base64::Engine;
        let dir = tempfile::tempdir().unwrap();
        let store = SingleProjectStore::new(project_at_path("blog", dir.path()));

        let encoded = base64::engine::general_purpose::STANDARD.encode([0xff_u8, 0xfe, 0x00, 0x01]);
        let args = WriteFileArgs {
            slug: "blog".into(),
            path: "bin.dat".into(),
            content: encoded,
            encoding: Some("base64".into()),
            create_parents: true,
        };
        let view = write_file_impl(&store, args).await.unwrap();
        assert_eq!(view.bytes_written, 4);
        let on_disk = std::fs::read(dir.path().join("bin.dat")).unwrap();
        assert_eq!(on_disk, vec![0xff, 0xfe, 0x00, 0x01]);
    }

    #[tokio::test]
    async fn write_file_rejects_path_escape() {
        let dir = tempfile::tempdir().unwrap();
        let store = SingleProjectStore::new(project_at_path("blog", dir.path()));

        let args = WriteFileArgs {
            slug: "blog".into(),
            path: "../outside.txt".into(),
            content: "nope".into(),
            encoding: None,
            create_parents: true,
        };
        let err = write_file_impl(&store, args).await.unwrap_err();
        // Path-escape errors map to MCP INVALID_PARAMS. Confirm code field is INVALID_INPUT
        // (matches the AppError::InvalidInput -> code mapping in mcp::errors).
        let code = data_code(&err);
        assert_eq!(
            code, "INVALID_INPUT",
            "got code {code} (expected INVALID_INPUT)"
        );
    }

    #[tokio::test]
    async fn write_file_refuses_to_overwrite_existing_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("im-a-dir")).unwrap();
        let store = SingleProjectStore::new(project_at_path("blog", dir.path()));

        let args = WriteFileArgs {
            slug: "blog".into(),
            path: "im-a-dir".into(),
            content: "should-be-rejected".into(),
            encoding: None,
            create_parents: true,
        };
        let err = write_file_impl(&store, args).await.unwrap_err();
        assert!(err.message.contains("directory") || data_code(&err) == "INVALID_INPUT");
    }

    #[test]
    fn build_exec_wrapper_quotes_command() {
        let wrapper = build_exec_wrapper(
            "opnble-proj-1",
            "echo 'hi there'; rm -rf /",
            Some("src"),
            &[("FOO".to_string(), "bar".to_string())],
            60,
        );
        assert!(wrapper.starts_with("timeout 60s podman exec"));
        assert!(wrapper.contains("opnble-proj-1"));
        assert!(wrapper.contains("-w /app/src"));
        assert!(wrapper.contains("-e FOO=bar"));
        assert!(wrapper.contains("rm -rf /"));
        assert_eq!(wrapper.matches("sh -c").count(), 1);
    }

    #[test]
    fn build_exec_wrapper_handles_empty_env_and_no_cwd() {
        let wrapper = build_exec_wrapper("opnble-blog", "npm test", None, &[], 30);
        assert!(wrapper.starts_with("timeout 30s podman exec"));
        assert!(!wrapper.contains("-w "));
        assert!(!wrapper.contains("-e "));
    }

    // === Phase 4.5: exec_command ===

    struct StubExec {
        stdout: String,
        stderr: String,
        exit_code: i32,
    }

    #[async_trait::async_trait]
    impl crate::mcp::exec::ExecRunner for StubExec {
        async fn run(
            &self,
            _wrapped: &str,
        ) -> Result<(i32, String, String), crate::error::AppError> {
            Ok((self.exit_code, self.stdout.clone(), self.stderr.clone()))
        }
    }

    #[tokio::test]
    async fn exec_command_returns_stdout_and_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let store = SingleProjectStore::new(project_at_path("blog", dir.path()));
        let exec = StubExec {
            stdout: "ok\n".into(),
            stderr: String::new(),
            exit_code: 0,
        };

        let args = crate::mcp::tools::types::ExecCommandArgs {
            slug: "blog".into(),
            cmd: "echo ok".into(),
            cwd: None,
            env: std::collections::HashMap::new(),
            timeout_ms: 5_000,
        };
        let view = exec_command_impl(&store, &exec, args).await.unwrap();
        assert_eq!(view.exit_code, 0);
        assert_eq!(view.stdout, "ok\n");
        assert!(!view.truncated);
    }

    #[tokio::test]
    async fn exec_command_truncates_large_stdout() {
        let dir = tempfile::tempdir().unwrap();
        let store = SingleProjectStore::new(project_at_path("blog", dir.path()));
        let exec = StubExec {
            stdout: "x".repeat(200_000),
            stderr: String::new(),
            exit_code: 0,
        };
        let args = crate::mcp::tools::types::ExecCommandArgs {
            slug: "blog".into(),
            cmd: "yes".into(),
            cwd: None,
            env: std::collections::HashMap::new(),
            timeout_ms: 5_000,
        };
        let view = exec_command_impl(&store, &exec, args).await.unwrap();
        assert!(view.truncated);
        assert!(view.stdout.len() <= 131_072);
    }

    #[tokio::test]
    async fn exec_command_rejects_non_ready_project() {
        let dir = tempfile::tempdir().unwrap();
        let mut project = make_project(
            "proj-test",
            Some("blog"),
            ProjectStatus::Stopped,
            None,
            true,
        );
        project.local_path = dir.path().to_string_lossy().into_owned();
        let store = SingleProjectStore::new(project);
        let exec = StubExec {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
        };

        let args = crate::mcp::tools::types::ExecCommandArgs {
            slug: "blog".into(),
            cmd: "ls".into(),
            cwd: None,
            env: std::collections::HashMap::new(),
            timeout_ms: 5_000,
        };
        let err = exec_command_impl(&store, &exec, args).await.unwrap_err();
        let code = data_code(&err);
        // `not_running_error` returns code "NOT_RUNNING"; assertion is lenient
        // so a future rename to PROJECT_NOT_RUNNING does not silently break it.
        assert!(
            code == "NOT_RUNNING"
                || code == "PROJECT_NOT_RUNNING"
                || err.message.contains("not running"),
            "expected NOT_RUNNING-shaped error, got code={code} msg={msg}",
            msg = err.message
        );
    }

    // === Phase 5.2: snapshot_project ===

    #[tokio::test]
    async fn snapshot_project_clones_tree_and_persists_record() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("hello.txt"), b"world").unwrap();

        let mut project = make_project(
            "proj-1",
            Some("blog"),
            ProjectStatus::Ready,
            Some(3001),
            true,
        );
        project.local_path = repo.to_string_lossy().into_owned();
        let store = SingleProjectStore::with_project(project);

        let snap_root = dir.path().join("snaps");
        let args = crate::mcp::tools::types::SnapshotArgs {
            slug: "blog".into(),
            label: Some("before-refactor".into()),
        };
        let view = snapshot_project_impl(&store, args, &snap_root)
            .await
            .unwrap();

        assert!(view.snapshot_id.starts_with("snap-"));
        assert_eq!(view.label.as_deref(), Some("before-refactor"));

        // Verify file on disk under snap_root/proj-1/<snap_id>/hello.txt
        let snap_path = snap_root
            .join("proj-1")
            .join(&view.snapshot_id)
            .join("hello.txt");
        assert_eq!(std::fs::read(snap_path).unwrap(), b"world");

        // Verify store has the record
        let pid = ProjectId::new("proj-1").unwrap();
        let snapshots = store.list_snapshots(&pid).unwrap();
        assert_eq!(snapshots.len(), 1);
        let first = snapshots.first().expect("one snapshot");
        assert_eq!(first.label.as_deref(), Some("before-refactor"));
    }

    // === Phase 5.4: rollback_project ===

    struct NoopOrch {
        #[allow(
            clippy::disallowed_types,
            reason = "lock never held across await; sync trait method"
        )]
        stop_called: std::sync::Mutex<bool>,
        #[allow(
            clippy::disallowed_types,
            reason = "lock never held across await; sync trait method"
        )]
        start_called: std::sync::Mutex<bool>,
    }

    impl NoopOrch {
        fn new() -> Self {
            #[allow(
                clippy::disallowed_types,
                reason = "lock never held across await; sync trait method"
            )]
            let stop_called = std::sync::Mutex::new(false);
            #[allow(
                clippy::disallowed_types,
                reason = "lock never held across await; sync trait method"
            )]
            let start_called = std::sync::Mutex::new(false);
            Self {
                stop_called,
                start_called,
            }
        }
    }

    #[async_trait::async_trait]
    impl crate::mcp::orch::OrchestratorOps for NoopOrch {
        async fn force_stop_and_remove(&self, _: &str) -> Result<(), crate::error::AppError> {
            *self.stop_called.lock().unwrap() = true;
            Ok(())
        }
        async fn start_project(
            &self,
            _: &tauri::AppHandle,
            _: &str,
            _: u16,
        ) -> Result<(), crate::error::AppError> {
            *self.start_called.lock().unwrap() = true;
            Ok(())
        }
    }

    #[tokio::test]
    async fn rollback_project_restores_files_from_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let snap_root = dir.path().join("snaps");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("file.txt"), b"v1").unwrap();

        let mut project = make_project("proj-1", Some("blog"), ProjectStatus::Ready, None, true);
        project.local_path = repo.to_string_lossy().into_owned();
        let store = SingleProjectStore::with_project(project);

        // Snapshot first.
        let snap_args = crate::mcp::tools::types::SnapshotArgs {
            slug: "blog".into(),
            label: None,
        };
        let snap_view = snapshot_project_impl(&store, snap_args, &snap_root)
            .await
            .unwrap();

        // Mutate the file (simulate the agent breaking it).
        std::fs::write(repo.join("file.txt"), b"v2-BROKEN").unwrap();
        assert_eq!(std::fs::read(repo.join("file.txt")).unwrap(), b"v2-BROKEN");

        // Rollback. Project port is None so the AppHandle isn't needed.
        let orch = NoopOrch::new();
        let rollback_args = crate::mcp::tools::types::RollbackArgs {
            slug: "blog".into(),
            snapshot_id: snap_view.snapshot_id.clone(),
        };
        let view = rollback_project_impl(&store, &orch, None, rollback_args, &snap_root)
            .await
            .unwrap();

        // File restored.
        assert_eq!(std::fs::read(repo.join("file.txt")).unwrap(), b"v1");
        // Orchestrator was told to stop.
        assert!(*orch.stop_called.lock().unwrap());
        // start_project was NOT called (port was None).
        assert!(!*orch.start_called.lock().unwrap());
        assert_eq!(view.snapshot_id, snap_view.snapshot_id);
    }

    // === Phase 6.2: fork_project ===

    /// Multi-project in-memory store for fork tests. `SingleProjectStore`
    /// holds exactly one project, but a fork inserts a second; this stub
    /// owns a `Vec<Project>` so `add`/`get`/`get_by_slug` all see the new
    /// row. `allocate_port` walks a monotonically increasing counter.
    struct MultiProjectStore {
        #[allow(
            clippy::disallowed_types,
            reason = "lock never held across await; sync trait method"
        )]
        projects: std::sync::Mutex<Vec<Project>>,
        #[allow(
            clippy::disallowed_types,
            reason = "lock never held across await; sync trait method"
        )]
        next_port: std::sync::Mutex<u16>,
    }

    impl MultiProjectStore {
        fn with_projects(projects: Vec<Project>) -> Self {
            #[allow(
                clippy::disallowed_types,
                reason = "lock never held across await; sync trait method"
            )]
            let projects = std::sync::Mutex::new(projects);
            #[allow(
                clippy::disallowed_types,
                reason = "lock never held across await; sync trait method"
            )]
            let next_port = std::sync::Mutex::new(3100);
            Self {
                projects,
                next_port,
            }
        }
    }

    impl ProjectStore for MultiProjectStore {
        fn list(&self) -> Result<Vec<Project>, crate::error::AppError> {
            Ok(self.projects.lock().expect("poisoned").clone())
        }
        fn get(&self, id: &ProjectId) -> Result<Project, crate::error::AppError> {
            self.projects
                .lock()
                .expect("poisoned")
                .iter()
                .find(|p| &p.id == id)
                .cloned()
                .ok_or_else(|| crate::error::AppError::NotFound {
                    entity: "project".into(),
                    id: id.to_string(),
                })
        }
        fn add(&self, project: Project) -> Result<(), crate::error::AppError> {
            self.projects.lock().expect("poisoned").push(project);
            Ok(())
        }
        fn update(&self, project: Project) -> Result<(), crate::error::AppError> {
            let mut projects = self.projects.lock().expect("poisoned");
            if let Some(slot) = projects.iter_mut().find(|p| p.id == project.id) {
                *slot = project;
            }
            Ok(())
        }
        fn remove(&self, _id: &ProjectId) -> Result<(), crate::error::AppError> {
            Ok(())
        }
        fn allocate_port(&self) -> Result<u16, crate::error::AppError> {
            let mut port = self.next_port.lock().expect("poisoned");
            let allocated = *port;
            *port += 1;
            Ok(allocated)
        }
        fn next_port(&self) -> Result<u16, crate::error::AppError> {
            Ok(*self.next_port.lock().expect("poisoned"))
        }
        fn add_snapshot(
            &self,
            _record: crate::snapshots::SnapshotRecord,
        ) -> Result<(), crate::error::AppError> {
            Ok(())
        }
        fn list_snapshots(
            &self,
            _project_id: &ProjectId,
        ) -> Result<Vec<crate::snapshots::SnapshotRecord>, crate::error::AppError> {
            Ok(Vec::new())
        }
        fn remove_snapshot(
            &self,
            id: &crate::snapshots::SnapshotId,
        ) -> Result<(), crate::error::AppError> {
            Err(crate::error::AppError::NotFound {
                entity: "snapshot".into(),
                id: id.as_str().to_string(),
            })
        }
        fn get_snapshot(
            &self,
            id: &crate::snapshots::SnapshotId,
        ) -> Result<crate::snapshots::SnapshotRecord, crate::error::AppError> {
            Err(crate::error::AppError::NotFound {
                entity: "snapshot".into(),
                id: id.as_str().to_string(),
            })
        }
    }

    #[tokio::test]
    async fn fork_project_creates_child_with_parent_id() {
        let dir = tempfile::tempdir().unwrap();
        let parent_repo = dir.path().join("parent");
        let repos_root = dir.path().join("repos");
        std::fs::create_dir_all(&parent_repo).unwrap();
        std::fs::write(parent_repo.join("file.txt"), b"parent-content").unwrap();

        let parent_id = ProjectId::new("proj-parent").unwrap();
        let mut parent = make_project(
            "proj-parent",
            Some("my-blog"),
            ProjectStatus::Ready,
            Some(3001),
            true,
        );
        parent.local_path = parent_repo.to_string_lossy().into_owned();
        let store = MultiProjectStore::with_projects(vec![parent.clone()]);

        let orch = NoopOrch::new();
        let args = crate::mcp::tools::types::ForkArgs {
            slug: "my-blog".into(),
            count: 1,
            label: None,
        };
        // No AppHandle in test; orchestrator stub is never called.
        let children = fork_project_impl(&store, &orch, None, args, &repos_root)
            .await
            .unwrap();

        assert_eq!(children.len(), 1);
        let child = children.first().expect("one child");
        assert!(child.slug.starts_with("my-blog-"));
        assert_eq!(child.parent_slug, "my-blog");

        // Store now has 2 projects.
        let projects = store.list().unwrap();
        assert_eq!(projects.len(), 2);
        let child_proj = projects
            .iter()
            .find(|p| p.slug.as_deref() == Some(&child.slug))
            .expect("child registered in store");
        assert_eq!(child_proj.parent_project_id.as_ref(), Some(&parent_id));
        assert!(child_proj.forked_at.is_some());
        assert!(child_proj.installed, "child inherits installed from parent");

        // Child tree on disk carries the parent's payload.
        assert_eq!(
            std::fs::read(child_proj.repo_path().join("file.txt")).unwrap(),
            b"parent-content"
        );

        // .opnble marker on the child carries the child slug.
        let marker_path = child_proj.repo_path().join(".opnble");
        assert!(marker_path.exists(), "child has .opnble marker");
        let marker_body = std::fs::read_to_string(&marker_path).unwrap();
        assert!(
            marker_body.contains(&child.slug),
            "marker carries child slug, got: {marker_body}"
        );

        // start_project was NOT called because AppHandle was None.
        assert!(!*orch.start_called.lock().unwrap());
    }

    #[tokio::test]
    async fn fork_project_rejects_non_ready_parent() {
        let dir = tempfile::tempdir().unwrap();
        let parent_repo = dir.path().join("parent");
        let repos_root = dir.path().join("repos");
        std::fs::create_dir_all(&parent_repo).unwrap();

        let mut parent = make_project(
            "proj-parent",
            Some("my-blog"),
            ProjectStatus::Stopped,
            None,
            true,
        );
        parent.local_path = parent_repo.to_string_lossy().into_owned();
        let store = MultiProjectStore::with_projects(vec![parent]);
        let orch = NoopOrch::new();

        let args = crate::mcp::tools::types::ForkArgs {
            slug: "my-blog".into(),
            count: 1,
            label: None,
        };
        let err = fork_project_impl(&store, &orch, None, args, &repos_root)
            .await
            .unwrap_err();
        assert!(
            err.message.contains("not running")
                || err.message.contains("Ready")
                || data_code(&err) == "NOT_RUNNING",
            "expected NOT_RUNNING-shaped error, got code={code} msg={msg}",
            code = data_code(&err),
            msg = err.message,
        );
    }
}
