//! Streamable-HTTP MCP server skeleton.
//!
//! `OpnbleMcp` is the rmcp `ServerHandler`. Phase 1 wires it up with the
//! bearer-token middleware and a placeholder discovery tool router; the
//! actual tool bodies arrive in Phase 2 onward.
//!
//! Lifetime: `spawn` is called once from the Tauri `setup` hook. It binds
//! `127.0.0.1:47821` and returns a `JoinHandle` whose future drives the
//! axum server forever (or until graceful shutdown).

use std::sync::Arc;

use axum::Router;
use rmcp::{
    handler::server::router::tool::ToolRouter,
    model::{ServerCapabilities, ServerInfo},
    tool_handler,
    transport::streamable_http_server::{
        session::local::LocalSessionManager, tower::StreamableHttpService,
    },
    ServerHandler,
};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex, RwLock};
use tokio::task::JoinHandle;

use crate::auth::{Authenticator, GitHubReposClient};
use crate::error::AppError;
use crate::infrastructure::git::GitOps;
use crate::infrastructure::runtime::Runtime;
use crate::mcp::exec::ExecRunner;
use crate::mcp::orch::OrchestratorOps;
use crate::mcp::{auth, config};
use crate::projects::store::ProjectStore;
use crate::projects::{ProjectOrchestrator, ProjectSyncer};
use crate::publish::github_pages::GitHubPagesClient;
use crate::tunnel::TunnelCoordinator;

/// Dependency bundle for [`OpnbleMcp`]. Constructed once in `spawn` so each
/// rmcp session clone is a cheap `Arc` clone (no `AppHandle` plumbing inside
/// individual tool calls).
///
/// **Field-order convention.** New dependencies are appended immediately
/// after `pages` and before `app_handle`, so `app_handle` stays anchored
/// at the end of the literal and concurrent edits touch different lines
/// of context. Reorder only with intent; an inconsistent order does not
/// break behavior but does break diff hygiene.
pub struct McpDeps {
    pub store: Arc<dyn ProjectStore>,
    pub orchestrator: Arc<ProjectOrchestrator>,
    pub runtime: Arc<dyn Runtime>,
    pub git: Arc<dyn GitOps>,
    pub authenticator: Arc<Authenticator>,
    pub tunnel: Arc<TunnelCoordinator>,
    pub pages: Arc<GitHubPagesClient>,
    pub repos: Arc<GitHubReposClient>,
    /// Lazy `agent_client::exec_host` wrapper used by `exec_command`. Holds
    /// no live agent until the VM starts (the production impl resolves it
    /// from `VmLifecycle`'s shared mutex at call time).
    pub exec: Arc<dyn ExecRunner>,
    /// Orchestrator trait seam used by `rollback_project` and
    /// `fork_project`; lets tool-body tests substitute a stub without a
    /// live `ProjectOrchestrator`.
    pub orch: Arc<dyn OrchestratorOps>,
    /// In-memory ring buffer of recent tool invocations. Every `#[tool]`
    /// wrapper records its outcome here; the Cursor-integration settings
    /// panel renders the most recent entries via `mcp_recent_tool_invocations`.
    pub ring: Arc<crate::mcp::ring::ToolRing>,
    pub app_handle: tauri::AppHandle,
    /// Periodic + on-stop sync worker pool. Threaded into the lifecycle
    /// MCP tools so `start_project` / `stop_project` / `restart_project`
    /// stay in sync with the matching Tauri commands.
    pub syncer: Arc<ProjectSyncer>,
}

impl McpDeps {
    /// Pull every dep [`OpnbleMcp`] needs out of Tauri's `manage()` state.
    /// Mirrors the Phase 0a `publish::commands` pattern (`state.inner()` ->
    /// `as_ref()`). Cheap: each `Arc::clone` is a refcount bump.
    pub fn collect(app: &tauri::AppHandle) -> Self {
        use tauri::Manager;
        let orchestrator: Arc<ProjectOrchestrator> =
            Arc::clone(app.state::<Arc<ProjectOrchestrator>>().inner());
        let orch: Arc<dyn OrchestratorOps> = Arc::new(crate::mcp::orch::LiveOrchestrator::new(
            Arc::clone(&orchestrator),
        ));
        Self {
            store: Arc::clone(app.state::<Arc<dyn ProjectStore>>().inner()),
            orchestrator,
            runtime: Arc::clone(app.state::<Arc<dyn Runtime>>().inner()),
            git: Arc::clone(app.state::<Arc<dyn GitOps>>().inner()),
            authenticator: Arc::clone(app.state::<Arc<Authenticator>>().inner()),
            tunnel: Arc::clone(app.state::<Arc<TunnelCoordinator>>().inner()),
            pages: Arc::clone(app.state::<Arc<GitHubPagesClient>>().inner()),
            repos: Arc::clone(app.state::<Arc<GitHubReposClient>>().inner()),
            exec: Arc::clone(app.state::<Arc<dyn ExecRunner>>().inner()),
            orch,
            ring: Arc::clone(app.state::<Arc<crate::mcp::ring::ToolRing>>().inner()),
            app_handle: app.clone(),
            syncer: Arc::clone(app.state::<Arc<ProjectSyncer>>().inner()),
        }
    }
}

/// The MCP `ServerHandler` for Opnble.
///
/// rmcp clones this once per session, so every `Arc` is held by reference
/// from the session's perspective. The tool methods live in
/// `crate::mcp::tools::discovery` under a single `#[tool_router]` impl on
/// this type.
///
/// Several fields are consumed only by specific tools (`git` by the git
/// tools, `pages`/`repos` by the GitHub tools, `tunnel` by the preview-link
/// tools); the struct still holds all of them so a single `Arc` clone in
/// `McpDeps::collect` covers every tool family without a per-tool dep
/// bundle. The `dead_code` allow below documents which fields are read by
/// which tools so a contributor can tell whether an unused-field warning
/// reflects real dead code or just the family they are touching.
#[allow(
    dead_code,
    reason = "tools are read by their family: git/authenticator by the git tools, tunnel by the preview-link tools, pages/repos by the GitHub tools, syncer by the lifecycle tools, exec by exec_command, orch by rollback_project/fork_project, ring by every tool wrapper, store/orchestrator/runtime/app_handle by all"
)]
#[derive(Clone)]
pub struct OpnbleMcp {
    /// Persisted project list (`state.json`).
    pub store: Arc<dyn ProjectStore>,
    /// VM / container lifecycle: start, stop, transition tracking.
    pub orchestrator: Arc<ProjectOrchestrator>,
    /// Platform container runtime abstraction.
    pub runtime: Arc<dyn Runtime>,
    /// Git read/write surface used by import / sync / publish.
    pub git: Arc<dyn GitOps>,
    /// OAuth tokens + provider clients.
    pub authenticator: Arc<Authenticator>,
    /// Cloudflare tunnel manager (`share` tool will use this).
    pub tunnel: Arc<TunnelCoordinator>,
    /// GitHub Pages REST client (used by the `github_pages_*` tools).
    pub pages: Arc<GitHubPagesClient>,
    /// GitHub repos REST client (used by the `github_push` tool's
    /// `create_repo_if_missing` flow).
    pub repos: Arc<GitHubReposClient>,
    /// `agent_client::exec_host` indirection used by the `exec_command`
    /// tool; the trait seam lets tool-body tests substitute a stub without
    /// a live VM.
    pub exec: Arc<dyn ExecRunner>,
    /// `ProjectOrchestrator` indirection used by `rollback_project` and
    /// `fork_project`; the trait seam lets tool-body tests substitute a
    /// stub without a live orchestrator.
    pub orch: Arc<dyn OrchestratorOps>,
    /// In-memory ring of recent tool invocations. Recorded on every
    /// `#[tool]` wrapper return via [`OpnbleMcp::record_outcome`].
    pub ring: Arc<crate::mcp::ring::ToolRing>,
    /// Forwarded to tools that emit Tauri events (logs, status updates).
    pub app_handle: tauri::AppHandle,
    /// Periodic + on-stop sync worker pool, mirroring the Tauri lifecycle
    /// commands so MCP `start_project` / `stop_project` / `restart_project`
    /// produce the same side effects as their UI counterparts.
    pub syncer: Arc<ProjectSyncer>,
}

impl OpnbleMcp {
    /// Move every field out of [`McpDeps`] into a fresh handler.
    pub fn from_deps(deps: McpDeps) -> Self {
        let McpDeps {
            store,
            orchestrator,
            runtime,
            git,
            authenticator,
            tunnel,
            pages,
            repos,
            exec,
            orch,
            ring,
            app_handle,
            syncer,
        } = deps;
        Self {
            store,
            orchestrator,
            runtime,
            git,
            authenticator,
            tunnel,
            pages,
            repos,
            exec,
            orch,
            ring,
            app_handle,
            syncer,
        }
    }

    /// Record a tool invocation outcome in the ring buffer. Called by every
    /// `#[tool]` wrapper on return so the Cursor-integration settings panel
    /// can render the most recent calls.
    ///
    /// `outcome` is `"ok"` on success or `"err:CODE"` where CODE is the
    /// `data.code` field of the `McpError` (defaults to `"UNKNOWN"` when the
    /// error carries no structured data, which should not happen for errors
    /// produced via [`crate::mcp::errors::into_mcp`]).
    pub(crate) fn record_outcome<T>(
        &self,
        tool: &str,
        slug: &str,
        result: &Result<T, rmcp::ErrorData>,
    ) {
        let outcome = match result {
            Ok(_) => "ok".to_string(),
            Err(err) => {
                let code = err
                    .data
                    .as_ref()
                    .and_then(|v| v.as_object())
                    .and_then(|o| o.get("code"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("UNKNOWN");
                format!("err:{code}")
            }
        };
        self.ring.record(tool, slug, outcome);
    }
}

/// `#[tool_handler]` reads the router with `Self::tool_router()`, which is
/// emitted by `#[tool_router]` over in `tools::discovery`. The macro also
/// generates `call_tool` / `list_tools`. We override `get_info` so the
/// announced capabilities advertise tools only (no resources or prompts in
/// Phase 1).
#[tool_handler(router = OpnbleMcp::tool_router())]
impl ServerHandler for OpnbleMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }
}

/// Force-import the discovery module so its `#[tool_router]` block is
/// compiled even though nothing names a function inside it directly.
const _: fn() -> ToolRouter<OpnbleMcp> = OpnbleMcp::tool_router;

/// Bind the loopback MCP socket on `127.0.0.1:47821`, returning `None` (after
/// logging) when the port is already held.
///
/// Called as the first I/O step of startup so the kernel begins accepting
/// connections into the listen backlog immediately, before the slower
/// dependency wiring and [`spawn`] run. That collapses the window where a
/// freshly relaunched Opnble would refuse the editor's MCP connection
/// (`ECONNREFUSED`), which the Cursor client treats as a non-retryable
/// failure: a queued connection is answered once [`spawn`] starts serving,
/// a refused one parks the client until the user reconnects by hand.
pub async fn bind_listener() -> Option<TcpListener> {
    bind_at(config::bind_address()).await
}

/// Bind helper split from [`bind_listener`] so tests can target an ephemeral
/// port instead of the fixed loopback address.
async fn bind_at(addr: std::net::SocketAddr) -> Option<TcpListener> {
    match TcpListener::bind(addr).await {
        Ok(listener) => {
            tracing::info!(
                "mcp listener bound on http://{addr}{path}",
                path = config::MCP_PATH
            );
            Some(listener)
        }
        Err(e) => {
            tracing::error!("cannot bind mcp listener on {addr}: {e}");
            None
        }
    }
}

/// Start serving MCP over the pre-bound `listener` in a background task.
/// Returns the `SharedToken` the bearer-auth middleware reads (so
/// `mcp_rotate_token` can update the live token in lockstep with disk) and an
/// [`McpHandle`] the exit hook drains on shutdown.
///
/// The listener is bound earlier by [`bind_listener`]; serving is deferred to
/// here only because the tool router needs the fully wired [`McpDeps`]. The
/// one fallible step is loading the bearer token; the bootstrap hook logs and
/// continues so the rest of Tauri still starts (the app works without MCP,
/// just without agent control).
pub async fn spawn(deps: McpDeps, listener: TcpListener) -> Result<SpawnedMcp, AppError> {
    let token = auth::current_token()?;
    let shared: auth::SharedToken = Arc::new(RwLock::new(token));
    let handler = OpnbleMcp::from_deps(deps);
    let app = build_router(handler, Arc::clone(&shared));

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let handle = tokio::spawn(async move {
        let served = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        if let Err(e) = served.await {
            tracing::error!("mcp listener exited: {e}");
        }
    });

    Ok(SpawnedMcp {
        handle,
        token: shared,
        shutdown: shutdown_tx,
    })
}

/// Result of [`spawn`]: the serve task, the live token handle for
/// `mcp_rotate_token`, and the shutdown trigger consumed by [`McpHandle`].
pub struct SpawnedMcp {
    pub handle: JoinHandle<()>,
    pub token: auth::SharedToken,
    pub shutdown: oneshot::Sender<()>,
}

/// Tauri-managed teardown control for the MCP listener. Holds the
/// graceful-shutdown trigger and the serve task so the exit hook can stop the
/// listener cleanly (drain in-flight requests, then join) instead of letting
/// process death sever connections mid-flight.
pub struct McpHandle {
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl McpHandle {
    pub fn new(spawned: SpawnedMcp) -> Self {
        Self {
            shutdown: Mutex::new(Some(spawned.shutdown)),
            task: Mutex::new(Some(spawned.handle)),
        }
    }

    /// Signal graceful shutdown and wait for the serve task to finish.
    /// Idempotent: the trigger and task are taken on first call, so a second
    /// call is a no-op.
    pub async fn shutdown(&self) {
        if let Some(tx) = self.shutdown.lock().await.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.lock().await.take() {
            let _ = task.await;
        }
    }
}

/// Compose the axum `Router` so the unit tests can exercise the wiring
/// without binding a TCP socket. Each new HTTP session gets its own clone of
/// [`OpnbleMcp`] via the `service_factory` closure.
fn build_router(handler: OpnbleMcp, token: auth::SharedToken) -> Router {
    let svc: StreamableHttpService<OpnbleMcp, LocalSessionManager> = StreamableHttpService::new(
        move || Ok(handler.clone()),
        Arc::new(LocalSessionManager::default()),
        rmcp::transport::streamable_http_server::tower::StreamableHttpServerConfig::default(),
    );

    Router::new()
        .nest_service(config::MCP_PATH, svc)
        .layer(axum::middleware::from_fn_with_state(
            token,
            auth::bearer_auth_layer,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn ephemeral() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)
    }

    #[tokio::test]
    async fn bind_at_returns_some_on_free_port() {
        let listener = bind_at(ephemeral()).await.expect("free port should bind");
        assert!(listener.local_addr().expect("addr").ip().is_loopback());
    }

    #[tokio::test]
    async fn bind_at_returns_none_when_port_taken() {
        let held = bind_at(ephemeral()).await.expect("first bind");
        let addr = held.local_addr().expect("addr");
        assert!(
            bind_at(addr).await.is_none(),
            "second bind on a held port must report failure, not panic"
        );
    }

    #[tokio::test]
    async fn mcp_handle_shutdown_joins_task_and_is_idempotent() {
        let (tx, rx) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let _ = rx.await;
        });
        let spawned = SpawnedMcp {
            handle: task,
            token: Arc::new(RwLock::new(String::new())),
            shutdown: tx,
        };
        let handle = McpHandle::new(spawned);
        handle.shutdown().await;
        handle.shutdown().await;
    }
}
