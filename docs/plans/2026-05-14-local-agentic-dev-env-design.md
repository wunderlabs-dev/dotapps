# Local agentic development environment: MCP server in Opnble

Status: design + implementation plan
Date: 2026-05-14
Owner: tbd
Related: alpha (no migration concerns)

## Goal

Let a Cursor agent (and any other MCP client) drive the operations a user already performs in the Opnble desktop UI: list projects, start and stop them, switch branches, pull, commit and push to GitHub, deploy to GitHub Pages, share a Cloudflare Tunnel preview link, and read recent logs.

The user lives inside a project (often imported from Lovable) opened in Cursor. The agent reads a small `.opnble` marker file at the repo root to learn the project's portable slug, then calls MCP tools that resolve that slug to the local Opnble project and act on it.

Non-goals for v1: external deploy targets (Vercel, Cloudflare Pages, Netlify), streaming logs, server-initiated progress notifications, per-action approval UI, importing brand-new repositories from the agent.

## Architecture

```
Cursor (agent)
   |  MCP over streamable HTTP (rmcp 1.7)
   |  Authorization: Bearer <local token>
   v
Opnble desktop app (Tauri host)
   |  axum router on 127.0.0.1:47821
   |  bearer-token middleware (axum::middleware::from_fn)
   |  StreamableHttpService /mcp -> ServerHandler (#[tool_router])
   |  tools call existing domain functions in-process
   v
ProjectStore + ProjectOrchestrator + GitOps + Authenticator
+ Runtime + TunnelCoordinator + GitHubPagesClient
+ NEW: GitHubReposClient (POST /user/repos)
```

Two new building blocks inside `src/tauri/src/`:

1. **`mcp/`** module: hosts the HTTP listener, bearer auth, slug resolution, install (write `.cursor/mcp.json` per project), and the tool implementations. Tools are thin adapters over existing domain code; no business logic duplicated.
2. **`.opnble` reader/writer** (small utility shared between the importer and the MCP slug resolver).

Plus targeted edits to existing modules:

- `src/tauri/src/projects/types.rs`: add `slug: Option<String>` to `Project`.
- `src/tauri/src/projects/importer.rs`: write `.opnble` and `.cursor/mcp.json` after clone, append to `.gitignore`.
- `src/tauri/src/infrastructure/git.rs`: add `status()` (porcelain) and extend `checkout` to optionally create.
- `src/tauri/src/infrastructure/runtime.rs`: add `tail_logs(project_id, lines)` to the trait and to all impls.
- `src/tauri/src/auth/`: add `GitHubReposClient` (or extend `Authenticator`) for `POST /user/repos`.
- `src/tauri/src/error.rs`: split `PublishFailed { reason: String }` into a structured `Publish(PublishError)` enum.
- `src/web/lib/errors.ts`: add the new error variants.
- `src/web/`: add a small "Cursor integration" Settings panel (status, rotate token, optional global install button).

## Background facts (verified)

- Commands are wired through `tauri-specta` in `src/tauri/src/app/register.rs`, not `tauri::generate_handler!`. Adding a non-Tauri-command module does not affect the parity script in `scripts/check-tauri-command-parity.sh`.
- `bootstrap.rs:101-110` already runs async setup (`resolve_cloudflared`) inside Tauri's `setup` closure after `app.manage()` calls. The MCP listener spawns from the same place.
- `~/.opnble/state.json` is the project registry, `JsonProjectStore` fronts it with an in-process `Mutex<StoredState>` and `AtomicJsonFile` writes.
- `ProjectId` is `proj-{unix_millis}`, local-only. `Project` has no portable slug today.
- `start_project` in the orchestrator persists state and spawns the reconciler; the reconciler probes `http://127.0.0.1:{port}/` every 3s and flips status to `Ready`. Status is broadcast on `project-status-{id}` Tauri events. To get block-until-ready in MCP, subscribe to that channel from the tool.
- No log buffer exists in Opnble. `project-log-{id}` is a defined event name with zero emitters and zero subscribers. Container engine (`podman logs`) is the source of truth.
- `Authenticator` already requests OAuth `repo` scope. `GitHubPagesClient` shows the existing pattern for a focused REST client (`reqwest` + `bearer_auth` + structured errors).
- specta TypeScript bindings (`src/web/gen/tauri.ts`) are regenerated only on macOS debug builds; Windows/Linux contributors need a macOS dev to refresh after AppError changes.

## Tool surface (v1: 15 tools)

All tools take `slug: string` and resolve it locally via `ProjectStore::list` filtered by `slug == arg`. Errors are `McpError` (JSON-RPC envelope) with `data: { code, hint? }` so the agent can recover programmatically.

| Group | Tool | Returns | Annotations |
|---|---|---|---|
| Discovery | `list_projects` | `[{ slug, name, id, repo, branch, status, port?, preview_url?, pages_url? }]` | read_only, idempotent |
| Discovery | `get_project({ slug })` | full project record | read_only, idempotent |
| Lifecycle | `start_project({ slug })` | `{ status, port, url }` (block until Ready) | destructive, idempotent |
| Lifecycle | `stop_project({ slug })` | `{ status }` (block until Stopped) | destructive, idempotent |
| Lifecycle | `restart_project({ slug })` | `{ status, port, url }` | destructive, idempotent |
| Logs | `tail_logs({ slug, lines? })` | `{ lines: string[], truncated: bool }` | read_only, idempotent |
| Git | `git_status({ slug })` | `{ branch, ahead, behind, dirty, files: [{ path, status }] }` | read_only, idempotent |
| Git | `switch_branch({ slug, branch, create? })` | `{ branch }` | destructive |
| Git | `pull_latest({ slug })` | `{ ahead, behind, conflicts? }` | destructive |
| Preview | `create_preview_link({ slug })` | `{ url }` | destructive, idempotent |
| Preview | `get_preview_link({ slug })` | `{ url } | null` | read_only, idempotent |
| Preview | `stop_preview_link({ slug })` | `{ stopped: true }` | destructive, idempotent |
| GitHub | `github_push({ slug, message, create_repo_if_missing?, visibility? })` | `{ remote_url, commit_sha, created_repo }` | destructive |
| Pages | `github_pages_publish({ slug })` | `{ url, framework, deployed_at }` | destructive |
| Pages | `github_pages_unpublish({ slug })` | `{ unpublished: true }` | destructive |

Server-side timeout: 5 minutes for everything except `github_pages_publish` (10 minutes; build + push + Pages deploy).

## `.opnble` marker

`.opnble` (committed JSON, at repo root, written at import time):

```json
{
  "version": 1,
  "slug": "my-blog",
  "repo": "https://github.com/owner/my-blog.git"
}
```

`slug` is the stable identity used by MCP tools. It is generated at import time from the repo URL (last path component, kebab-cased, deduplicated against existing slugs in `state.json`). `version` exists for forward evolution.

## `.cursor/mcp.json`

Per-project local config (gitignored, written at import time):

```json
{
  "mcpServers": {
    "opnble": {
      "url": "http://127.0.0.1:47821/mcp",
      "headers": { "Authorization": "Bearer <local-token>" }
    }
  }
}
```

Cursor watches this file and reloads MCP servers automatically.

## Implementation phases

Each phase is independently testable. Phases 0-2 are sequential; 3-7 can interleave once 2 lands.

### Phase 0: Foundations (refactors prerequisites)

**0a. AppError refactor** (`src/tauri/src/error.rs`, `src/web/lib/errors.ts`, `src/web/gen/tauri.ts`)

Replace `AppError::PublishFailed { reason: String }` with `AppError::Publish(PublishError)` where `PublishError` is a `thiserror`-derived enum with these variants:

```
PublishError::NoPackageJson
PublishError::NoBuildScript
PublishError::NextjsStaticExportRequired
PublishError::PrivateRepoNeedsPro
PublishError::RuntimeNotReady
PublishError::BuildFailed { tail: String }
PublishError::BuildOutputMissing { expected: String }
PublishError::PushFailed { reason: String }
PublishError::PagesApiFailed { status: u16, body: String }
```

Each variant must derive `Clone, Debug, Error, Serialize, specta::Type` and use `#[serde(tag = "kind", content = "detail")]` for the inner enum so the JSON shape stays predictable.

Update call sites in `src/tauri/src/publish/deployer.rs`, `src/tauri/src/publish/builder.rs`, `src/tauri/src/publish/github_pages.rs` to construct typed variants instead of `format!`-ing strings into `PublishFailed { reason }`.

Update `src/web/lib/errors.ts`:
- Add a `Publish` entry to `ERROR_MESSAGES` (top-level title) and a sub-mapper for the inner `kind`.
- Extend `extractDetail` to handle the nested shape.
- Add tests in `src/web/lib/errors.test.ts` for each new sub-variant.

Regenerate `src/web/gen/tauri.ts` (run on macOS, debug build, see `register.rs:82-89`).

**0b. Slug field on Project** (`src/tauri/src/projects/types.rs`, `src/tauri/src/projects/store.rs`)

Add `slug: Option<String>` to `Project`. Use `#[serde(default)]` so old state.json files still deserialize.

Add a helper to `Project` (or a free function in `projects::slug`):

```
pub fn derive_slug(repo_url: &str) -> String
```

Strategy: take the last path segment of the URL, strip `.git`, kebab-case, ASCII-only. Validate same as `ProjectId` (alphanumeric + `-`). On collision with an existing slug in the store, suffix `-2`, `-3`, etc.

Add to `ProjectStore` trait:

```
fn get_by_slug(&self, slug: &str) -> Result<Project, AppError>;
```

Default impl: linear scan over `list()`. On `NotFound`, return `AppError::NotFound { entity: "project".into(), id: slug.into() }`.

**0c. GitOps additions** (`src/tauri/src/infrastructure/git.rs`)

Add to the `GitOps` trait:

```
fn status(&self, repo_path: &Path) -> Result<RepoStatus, AppError>;
```

where:

```
pub struct RepoStatus {
    pub branch: String,
    pub ahead: usize,
    pub behind: usize,
    pub dirty: bool,
    pub files: Vec<FileStatus>,
}

pub struct FileStatus {
    pub path: String,
    pub status: FileChange,  // Added | Modified | Deleted | Untracked | Renamed
}
```

Implement on `LibGitClient` using libgit2's `Repository::statuses(None)` + `Repository::head()` + `Repository::graph_ahead_behind()`.

Extend the existing `checkout` to accept a `create: bool`:

```
fn checkout(&self, repo_path: &Path, branch: &str, create: bool) -> Result<(), AppError>;
```

When `create == true`, use libgit2 `Repository::branch()` from current `HEAD` if the branch doesn't exist, then check it out. Update existing call sites to pass `create: false`.

**0d. Runtime::tail_logs** (`src/tauri/src/infrastructure/runtime.rs` and per-platform impls)

Add to the `Runtime` trait:

```
async fn tail_logs(&self, project_id: &str, lines: u32) -> Result<Vec<String>, AppError>;
```

Implementations:

- `PodmanRuntime` (Linux): bollard `logs` with `tail = lines.to_string()`, `follow: false`, decode multiplexed frames into `Vec<String>`.
- `VmRuntime` (macOS): wrap existing `AgentClient::stream_logs(project_id, lines)`, map the `Vec<(String, String)>` (timestamp, line) into `Vec<String>` keeping the line content.
- `WSL2Runtime` (Windows): change `WSL2::logs(project_id)` to `WSL2::logs(project_id, lines: u32)` and replace the hardcoded `--tail 100` with `--tail {lines}`. Update existing call sites if any.
- `StubRuntime`: return `Err(AppError::ContainerFailed { reason: "runtime unavailable".into() })`.

**Verification for Phase 0**: `make check` passes (frontend + Rust), all existing tests still pass, and:
- `cargo test -p opnble publish::deployer::tests` covers the new typed errors.
- `cargo test -p opnble infrastructure::git::tests` covers `status()` and `checkout(create=true)` with a tempdir repo.
- New unit test for `derive_slug` and collision handling.

### Phase 1: MCP server skeleton (no tools yet)

Create `src/tauri/src/mcp/` with these files:

```
mcp/
  mod.rs              public init(app_handle, deps) -> JoinHandle, called from bootstrap.rs
  server.rs           OpnbleMcp struct, ServerHandler impl, axum Router setup
  auth.rs             token generate/load (~/.opnble/mcp.json), bearer middleware
  resolve.rs          slug -> Project resolution via ProjectStore::get_by_slug
  install.rs          write .cursor/mcp.json + patch .gitignore for a project path
  errors.rs           AppError -> McpError mapping (with code + hint in data)
  config.rs           constants: PORT (47821), MCP_PATH ("/mcp"), file paths
  tools/
    mod.rs            #[tool_router] glue
    discovery.rs      list_projects, get_project (placeholder returning Err for now)
```

**Cargo.toml** (root or `src/tauri/Cargo.toml`):

```toml
rmcp = { version = "1.7", features = [
  "server", "macros", "transport-streamable-http-server", "schemars"
] }
rmcp-macros = "1.7"
schemars = "1"
axum = "0.8"
```

**`mcp::auth`** responsibilities:

- Path: `~/.opnble/mcp.json` via `paths::data_dir().join("mcp.json")`. Reuse existing `paths` module.
- On first read: generate a 32-byte random token (`rand::thread_rng().fill_bytes` -> hex), write `{"url": "http://127.0.0.1:47821/mcp", "token": "..."}` with file mode `0o600` on Unix (skip on Windows).
- Public function `current_token() -> Result<String, AppError>`.
- Public function `rotate_token() -> Result<String, AppError>` (regenerates, sweeps `.cursor/mcp.json` files - see `install::sweep_token`).
- Bearer middleware `bearer_auth_layer(token: Arc<RwLock<String>>)`: returns `axum::middleware::from_fn` that reads `Authorization: Bearer <t>` and 401s on mismatch. Use an `Arc<RwLock<String>>` so token rotation takes effect without rebinding the listener.

**`mcp::resolve`** responsibilities:

- `pub fn project_for_slug(store: &dyn ProjectStore, slug: &str) -> Result<Project, AppError>`: calls `store.get_by_slug(slug)`.
- Maps `NotFound` to a typed `AppError::NotFound`.

**`mcp::server`** responsibilities:

- Define `OpnbleMcp` holding `Arc`s of every domain dep needed by tools: `ProjectStore`, `ProjectOrchestrator`, `Runtime`, `GitOps`, `Authenticator`, `TunnelCoordinator`, `GitHubPagesClient`, `GitHubReposClient`, `AppHandle` (for status event subscription).
- `#[tool_handler]` impl with `enable_tools()` only (no resources, no prompts).
- `pub async fn spawn(app: tauri::AppHandle, deps: McpDeps) -> Result<(), AppError>`:
  1. Generate or load token via `auth::current_token`.
  2. Build `StreamableHttpService::new(|| Ok(OpnbleMcp{...}), Arc::new(LocalSessionManager::default()), StreamableHttpServerConfig::default())`.
  3. Build `axum::Router::new().nest_service("/mcp", svc).layer(bearer_auth_layer(token))`.
  4. Bind `127.0.0.1:47821` (configurable via `OPNBLE_MCP_PORT` env for tests).
  5. Spawn `axum::serve(listener, app)` on `tauri::async_runtime::spawn`.

**`mcp::errors`** responsibilities:

- `pub fn into_mcp(err: AppError) -> rmcp::ErrorData`. Maps each `AppError` variant to a JSON-RPC error code and stuffs `{ code: "<STRING>", hint?: "..." }` into `data`.
- For each variant, choose the JSON-RPC numeric code: `-32602` for invalid params (`InvalidInput`, `NotFound`), `-32603` for internal (`Internal`, `ContainerFailed`, `GitFailed`, `StorageFailed`, `VmStartFailed`, etc.), and the structured `code` field is the human-readable identifier (`"PROJECT_NOT_FOUND"`, `"NO_GITHUB_AUTH"`, `"NEXTJS_STATIC_EXPORT_REQUIRED"`, ...).
- Hint generation is variant-specific; centralize in this module to keep tools clean.

**Hook into bootstrap** (`src/tauri/src/app/bootstrap.rs`):

After `resolve_cloudflared(app)` at line 110, add:

```rust
let mcp_deps = mcp::McpDeps::collect(&app.handle());  // pulls Arcs from State<>
mcp::server::spawn(app.handle().clone(), mcp_deps);
```

Wrap in `tauri::async_runtime::spawn` (mirror the cloudflared pattern). Errors during bind are logged but do not block app startup; user sees status in the Settings panel.

**Verification for Phase 1**:

- `make lint-rust` passes (rmcp + axum integrate cleanly under deny-clippy-pedantic).
- Unit tests in `mcp::auth`: token generation, file mode, idempotent re-read, rotate.
- Manual smoke: launch the app, then `curl -H "Authorization: Bearer $(jq -r .token ~/.opnble/mcp.json)" http://127.0.0.1:47821/mcp -X POST -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","clientInfo":{"name":"smoke","version":"0"},"capabilities":{}}}'`. Expect a successful `initialize` response.
- Without token: 401.
- Bind to `0.0.0.0` is rejected (assert in test that bind address is loopback).

### Phase 2: Discovery + lifecycle tools + slug resolution end-to-end

**`mcp/tools/discovery.rs`**:

```
#[tool(description = "List all projects in Opnble", annotations(read_only_hint = true, idempotent_hint = true))]
async fn list_projects(&self) -> Result<Json<ListProjectsOutput>, McpError>

#[tool(description = "Get details of a project by slug", annotations(read_only_hint = true, idempotent_hint = true))]
async fn get_project(&self, Parameters(args): Parameters<GetProjectArgs>) -> Result<Json<ProjectView>, McpError>
```

`ProjectView` is a serde+schemars struct that flattens `Project` fields the agent needs: `slug, name, id, repo, branch, status (string), port, preview_url, pages_url, last_synced_at`. Define in `mcp/tools/types.rs` to avoid leaking `Project` directly.

`list_projects` skips projects with `slug == None` (legacy, pre-MCP imports if any exist in alpha).

**`mcp/tools/lifecycle.rs`**:

```
#[tool(annotations(destructive_hint = true, idempotent_hint = true))]
async fn start_project(&self, Parameters({ slug }): Parameters<SlugOnly>) -> Result<Json<RunningView>, McpError>

#[tool(annotations(destructive_hint = true, idempotent_hint = true))]
async fn stop_project(&self, Parameters({ slug }): Parameters<SlugOnly>) -> Result<Json<StatusView>, McpError>

#[tool(annotations(destructive_hint = true, idempotent_hint = true))]
async fn restart_project(&self, Parameters({ slug }): Parameters<SlugOnly>) -> Result<Json<RunningView>, McpError>
```

**Block-until-ready implementation**:

The orchestrator emits `StatusEvent` on `events::project_status(project_id)`. To block:

1. Subscribe before triggering: `let mut rx = subscribe_status(&app, &project.id);`
2. Trigger: `orchestrator.start_project(&app, project.id.as_str(), port).await?`
3. Loop on `rx.recv()` with a `tokio::time::timeout(Duration::from_secs(300))`:
   - On `Ready`: return `RunningView { status: "running", port, url: format!("http://127.0.0.1:{port}/") }`.
   - On `Failed`: return `McpError` with `code: "START_FAILED"`, `hint: error_summary`.
   - On `WaitingForVm` for >60s: return early with `code: "VM_OFFLINE"`.
   - On overall timeout: `code: "START_TIMEOUT"` with `last_log_line` (call `runtime.tail_logs(id, 1)`).

Helper module `mcp/wait.rs`:

```
pub async fn wait_for_status(
    app: &AppHandle,
    project_id: &ProjectId,
    target: ProjectStatus,
    timeout: Duration,
) -> Result<StatusEvent, AppError>
```

Implements the subscribe + timeout pattern. Reused by start, stop, restart, and `pull_latest` (see Phase 4).

For `stop_project`, target is `Stopped`; `restart_project` chains `force_stop_and_remove` -> `allocate_port` -> `start_project` -> wait, mirroring `commands.rs:318-344`.

**Verification for Phase 2**:

- Connect Cursor to `http://127.0.0.1:47821/mcp` via a hand-edited `~/.cursor/mcp.json`. Confirm `list_tools` returns 5 tools and that calling `list_projects` returns the expected slugs.
- Integration test using `rmcp::model::CallToolRequestParam` against an in-process server (mock `ProjectStore`). Cover: slug resolution, missing slug, status wait timeout (use a fake orchestrator that never emits `Ready`).
- Manual: `start_project` from Cursor, confirm the tool blocks until the dev server URL responds, then returns the URL.

### Phase 3: Logs

**`mcp/tools/logs.rs`**:

```
#[tool(annotations(read_only_hint = true, idempotent_hint = true))]
async fn tail_logs(&self, Parameters(args): Parameters<TailLogsArgs>) -> Result<Json<TailLogsOutput>, McpError>

struct TailLogsArgs { slug: String, #[serde(default = "default_lines")] lines: u32 }
struct TailLogsOutput { lines: Vec<String>, truncated: bool }
```

`default_lines` returns 200; cap `args.lines` at 5000.

Implementation: resolve slug -> project -> `runtime.tail_logs(project.id.as_str(), args.lines).await`. `truncated` is `true` if the returned vec length equals the cap (heuristic; container engines don't always tell us if older lines exist).

**Verification for Phase 3**:

- Unit test for `cap_lines`.
- Integration test: start a project, call `tail_logs`, confirm non-empty output. (Requires a running runtime; gate behind `#[ignore]` for CI or a feature flag.)
- Cursor manual smoke: ask the agent "show me the last 20 lines of my-blog logs".

### Phase 4: Git tools

**`mcp/tools/git.rs`**:

```
#[tool(annotations(read_only_hint = true, idempotent_hint = true))]
async fn git_status(&self, Parameters({ slug }): Parameters<SlugOnly>) -> Result<Json<GitStatusView>, McpError>

#[tool(annotations(destructive_hint = true))]
async fn switch_branch(&self, Parameters(args): Parameters<SwitchBranchArgs>) -> Result<Json<BranchView>, McpError>
struct SwitchBranchArgs { slug: String, branch: String, #[serde(default)] create: bool }

#[tool(annotations(destructive_hint = true))]
async fn pull_latest(&self, Parameters({ slug }): Parameters<SlugOnly>) -> Result<Json<PullView>, McpError>
```

`git_status`: resolve project, call `git_ops.status(&project.repo_path())`, map `RepoStatus` to `GitStatusView` (camelCase fields).

`switch_branch`: resolve, call `git_ops.checkout(&project.repo_path(), &args.branch, args.create)`. On `DIRTY_TREE` from libgit2, return `McpError` with `code: "DIRTY_TREE"` and the file list in `hint` (call `git_status` first for context).

`pull_latest`: resolve, call `git_ops.pull(&project.repo_path(), token)`. Token via `authenticator.resolve_push_token(&project.repo_url).await?`. After pull, call `git_ops.status` to return the new ahead/behind.

**Verification for Phase 4**:

- Tempdir-based integration tests using a local bare repo as origin: cover clean status, dirty status, branch creation, pull with no remote changes, pull with conflicts.

### Phase 5: Preview link

**`mcp/tools/preview.rs`**:

```
#[tool(annotations(destructive_hint = true, idempotent_hint = true))]
async fn create_preview_link(...) -> Json<PreviewView>

#[tool(annotations(read_only_hint = true, idempotent_hint = true))]
async fn get_preview_link(...) -> Json<Option<PreviewView>>

#[tool(annotations(destructive_hint = true, idempotent_hint = true))]
async fn stop_preview_link(...) -> Json<{ stopped: bool }>
```

Implementations are direct adapters over `TunnelCoordinator::share / tunnel_url / unshare`, mirroring `tunnel/commands.rs:22-112`. Reuse the same preconditions (Ready status, port allocated). Token via `authenticator.resolve_push_token`.

**Verification for Phase 5**:

- Mocked `TunnelCoordinator` test: confirm `create` is idempotent, `get` returns `null` when no tunnel, `stop` is idempotent.
- Manual: from Cursor, "share my-blog" -> tunnel URL returned -> visit URL -> confirm dev server reachable.

### Phase 6: GitHub tools

**6a. New `GitHubReposClient`** (`src/tauri/src/auth/github_repos.rs`)

Modeled exactly on `publish/github_pages.rs`:

```
pub struct GitHubReposClient { http: reqwest::Client }

impl GitHubReposClient {
    pub fn new() -> Result<Self, AppError>;

    /// POST /user/repos. Returns the new repo's clone URL on success.
    pub async fn create_user_repo(
        &self,
        token: &str,
        name: &str,
        private: bool,
    ) -> Result<CreatedRepo, AppError>;

    /// GET /repos/{owner}/{repo}. Used to check if a repo exists by name.
    pub async fn repo_exists(
        &self,
        token: &str,
        owner: &str,
        name: &str,
    ) -> Result<bool, AppError>;
}

pub struct CreatedRepo {
    pub clone_url: String,
    pub html_url: String,
    pub default_branch: String,
}
```

Wrap reqwest errors in a local `RepoApiError` -> `AppError::AuthFailed { reason }` or a new variant `AppError::GitHubApi { status: u16, body: String }` (decide during 0a; either reuse `Internal` or add a typed variant). Manage in `bootstrap.rs` alongside `GitHubPagesClient`.

**6b. `mcp/tools/github.rs::github_push`**

```
#[tool(annotations(destructive_hint = true))]
async fn github_push(&self, Parameters(args): Parameters<GithubPushArgs>) -> Result<Json<PushView>, McpError>

struct GithubPushArgs {
    slug: String,
    message: String,
    #[serde(default = "default_true")]
    create_repo_if_missing: bool,
    #[serde(default = "default_visibility")]
    visibility: Visibility,  // Public | Private
}

struct PushView {
    remote_url: String,
    commit_sha: String,
    created_repo: bool,
}
```

Flow:

1. Resolve project, get `repo_path`.
2. `git_ops.status(&repo_path)`. If `dirty`, run `git_ops.commit_auto(&repo_path, &args.message)?` (existing trait method).
3. Determine remote state: open the repo via libgit2, check if `origin` exists.
4. If no `origin`:
   a. If `!args.create_repo_if_missing`: return `McpError { code: "NO_REMOTE" }`.
   b. Else: get token via `authenticator.valid_token("github")`. If missing: `code: "NO_GITHUB_AUTH"`. Get user via `authenticator.github_user(&token)`. Compute repo name from `project.slug`. Optional collision check via `repos_client.repo_exists`. Call `repos_client.create_user_repo(&token, &name, args.visibility == Private)`. Add `origin` remote pointing at `clone_url` (libgit2 `Repository::remote("origin", &clone_url)`).
5. `git_ops.push_current_branch(&repo_path, Some(&token))`. Map `PushOutcome` accordingly.
6. Read the new HEAD commit SHA via libgit2.
7. Return `PushView { remote_url, commit_sha, created_repo: <bool from step 4> }`.

This adds one `GitOps` method to keep adapter small:

```
fn add_remote(&self, repo_path: &Path, name: &str, url: &str) -> Result<(), AppError>;
fn current_commit_sha(&self, repo_path: &Path) -> Result<String, AppError>;
```

(Implement on `LibGitClient` with `Repository::remote()` and `Repository::head().peel_to_commit().id()`.)

**6c. `mcp/tools/github_pages.rs`**

Thin adapters over `publish::deployer::publish` and `publish::deployer::unpublish`:

```
#[tool(annotations(destructive_hint = true))]
async fn github_pages_publish(&self, Parameters({ slug }): Parameters<SlugOnly>) -> Result<Json<PublishedView>, McpError>

#[tool(annotations(destructive_hint = true, idempotent_hint = true))]
async fn github_pages_unpublish(&self, Parameters({ slug }): Parameters<SlugOnly>) -> Result<Json<{ unpublished: bool }>, McpError>
```

Construct `PublishDeps` / `UnpublishDeps` and call. Map `PublishError` variants (added in 0a) to MCP codes via `mcp::errors::into_mcp`. Tool ceiling 10 minutes, set in `mcp/config.rs` and applied per-tool via `tokio::time::timeout` in the wrapper.

**Verification for Phase 6**:

- `GitHubReposClient` tests with `wiremock` to fake the GitHub API (HTTP 201 success, 422 name taken, 401 unauthorized).
- Integration test for `github_push` using a tempdir local bare-repo `origin` to bypass network and `create_repo_if_missing: false`.
- Manual: import a Lovable project (no remote) into Opnble, then from Cursor "push to GitHub as a private repo and publish to Pages". Confirm the repo appears in GitHub, gh-pages branch is pushed, Pages URL is returned.

### Phase 7: Install integration (`.opnble` + `.cursor/mcp.json`)

**7a. Marker file utility** (`src/tauri/src/projects/marker.rs`)

```
pub struct Marker { pub version: u32, pub slug: String, pub repo: String }

pub fn write_marker(repo_path: &Path, marker: &Marker) -> Result<(), AppError>;
pub fn read_marker(repo_path: &Path) -> Result<Option<Marker>, AppError>;
```

`write_marker`: serializes JSON with two-space indent + trailing newline, writes `<repo>/.opnble`. Idempotent (overwrites).

`read_marker`: returns `None` if file missing, `Err` if file present but invalid.

**7b. Cursor MCP install** (`src/tauri/src/mcp/install.rs`)

```
pub fn write_project_mcp_config(repo_path: &Path, token: &str, port: u16) -> Result<(), AppError>;
pub fn append_to_gitignore(repo_path: &Path, line: &str) -> Result<(), AppError>;
pub fn sweep_token(store: &dyn ProjectStore, new_token: &str, port: u16) -> Result<(), AppError>;
```

`write_project_mcp_config`:
1. Path: `<repo>/.cursor/mcp.json`. Ensure `<repo>/.cursor/` exists.
2. If file exists: parse as JSON. If parse fails: log warning, return `Err(AppError::StorageFailed { reason: "existing .cursor/mcp.json is invalid JSON; refusing to overwrite" })`. Surface this to the UI as a "manual install needed" hint.
3. If file exists and parses: deep-merge `mcpServers.opnble = { url, headers: { Authorization: "Bearer <token>" } }` into the parsed value, preserving all other keys.
4. If file does not exist: write the full minimal JSON.
5. Write atomically via `AtomicJsonFile` (existing utility).

`append_to_gitignore`:
1. Path: `<repo>/.gitignore`.
2. Read file (or empty string if missing). If `line` already appears as a non-comment line, no-op.
3. Else append `line` with a leading newline if needed.
4. Write atomically.

We append two lines: `.cursor/mcp.json` and (defensively) `.cursor/mcp.local.json`.

`sweep_token`: iterate `store.list()`, for each project with `slug.is_some()` and a directory at `repo_path()`, call `write_project_mcp_config` with the new token. Best-effort: log per-project failures and continue.

**7c. Wire into importer** (`src/tauri/src/projects/importer.rs`)

After `git.clone_repo(...)` succeeds at line 36 (clone_result is bound), but before `store.add(project.clone())` at line 47:

1. `let slug = projects::slug::derive_slug_unique(&self.project, &project.repo_url)?;`
2. `let mut project = Project::new(...).with_slug(slug.clone());`  (constructor or builder change to accept the optional slug)
3. `marker::write_marker(&clone_result.local_path, &Marker { version: 1, slug: slug.clone(), repo: project.repo_url.clone() })?;`
4. `let token = mcp::auth::current_token()?;`
5. `mcp::install::write_project_mcp_config(&clone_result.local_path, &token, mcp::config::PORT)?;`
6. `mcp::install::append_to_gitignore(&clone_result.local_path, ".cursor/mcp.json")?;`
7. `self.project.add(project.clone())?;`

Errors in steps 3-6 should NOT block the import (the user still gets a working Opnble project). Surface as warnings via tracing and emit a non-fatal `StatusEvent::warning` if such a thing exists, else log only and continue.

Also handle `clone_repository` (the lower-level command at `commands.rs:441-471`) the same way since it's an alternate import entry. Extract the post-clone steps into a shared helper `projects::importer::after_clone_install(repo_path, slug, token)`.

**Verification for Phase 7**:

- Tempdir tests for `marker::{write,read}` and `install::{write_project_mcp_config, append_to_gitignore, sweep_token}`. Cover: existing file with other keys, existing file with comments (parse failure path), missing `.cursor/`, idempotent re-runs.
- Integration test: import a fixture repo, confirm `.opnble`, `.cursor/mcp.json`, and `.gitignore` are correct.
- Manual: import a fresh Lovable repo, open it in Cursor, confirm Cursor shows `opnble.*` tools without any user action.

### Phase 8: Frontend Settings panel

Add a "Cursor integration" section to the existing Settings UI in `src/web/`. Components:

- Status: green dot + "Listening on `127.0.0.1:47821`, N projects linked" (read N from a new Tauri command `mcp_status` that returns `{ url, port, linkedProjects }`).
- "Rotate token" button -> calls a new Tauri command `mcp_rotate_token` -> calls `mcp::auth::rotate_token` -> sweeps `.cursor/mcp.json`. The command returns `{ linkedProjects, failed }` so the panel can show a per-rotation summary like "Rotated. 7 updated, 0 failed."
- Optional "Connect to Cursor globally" link -> opens `cursor://anysphere.cursor-deeplink/mcp/install?name=opnble&config=<base64>` via `tauri::opener::open_url`. For users who want the MCP server in non-Opnble Cursor windows.

Two new Tauri commands (so the parity script needs updating to expect them):

```
#[tauri::command] #[specta::specta]
pub async fn mcp_status(...) -> Result<McpStatus, AppError>;
//   McpStatus { url: String, port: u16, linkedProjects: u32 }

#[tauri::command] #[specta::specta]
pub async fn mcp_rotate_token(...) -> Result<RotateReport, AppError>;
//   RotateReport { linkedProjects: u32, failed: u32 }
```

Add to `register.rs:10-53` shared list and update `scripts/check-tauri-command-parity.sh:44-146` expected lists.

**Verification for Phase 8**:

- `make check` passes including parity check.
- Frontend Vitest covers the panel (status display, rotate button calls, error states).
- Manual: rotate token, confirm Cursor reconnects without restart.

## Verification gates per phase

| Phase | Narrowest gate |
|---|---|
| 0a | `cd src/tauri && cargo test publish::` + `cd src/web && pnpm check` |
| 0b | `cargo test projects::store` + `cargo test projects::types` |
| 0c | `cargo test infrastructure::git` |
| 0d | `cargo test infrastructure::runtime` (per-platform `#[cfg]`-gated) |
| 1 | `cargo test mcp::auth` + manual `curl initialize` smoke |
| 2 | `cargo test mcp::tools::lifecycle` + Cursor end-to-end smoke |
| 3 | `cargo test mcp::tools::logs` |
| 4 | `cargo test mcp::tools::git` (tempdir bare repo) |
| 5 | `cargo test mcp::tools::preview` (mock TunnelCoordinator) |
| 6 | `cargo test auth::github_repos` (wiremock) + `cargo test mcp::tools::github` |
| 7 | `cargo test projects::marker` + `cargo test mcp::install` + import fixture test |
| 8 | `make check` (frontend + parity + clippy) |

After all phases: `make check` and `make test-all`, plus a manual end-to-end with a fresh Lovable import:
1. Open Opnble, import the Lovable URL.
2. Open the cloned folder in Cursor.
3. Ask the agent: "list my Opnble projects" -> verify slug appears.
4. "Start the project" -> verify URL returned.
5. "Push this to GitHub as a private repo and publish to Pages" -> verify both succeed.

## Cut-line for v1.1

Out of v1, in priority order for v1.1:

1. `import_project` MCP tool (let the agent clone a brand-new repo into Opnble).
2. `open_project_in_opnble` (focus desktop window on a project).
3. Server-initiated progress notifications for `start_project` and `github_pages_publish` (rmcp supports `RequestContext::peer.notify_progress`; agent UIs that render progress get a better UX).
4. Per-tool / tiered approval UI in the Opnble desktop app for destructive actions.
5. External deploy targets (Vercel, Cloudflare Pages, Netlify) - each is its own integration.
6. "Add to Cursor" badge in generated project READMEs.
7. Streaming `tail_logs` (would use the rmcp progress channel or follow a ring buffer).
8. Optional in-memory ring buffer of dev-server logs (only if `tail_logs` Podman snapshot proves insufficient).

## File-by-file change inventory

New files:

- `src/tauri/src/mcp/mod.rs`
- `src/tauri/src/mcp/server.rs`
- `src/tauri/src/mcp/auth.rs`
- `src/tauri/src/mcp/resolve.rs`
- `src/tauri/src/mcp/install.rs`
- `src/tauri/src/mcp/errors.rs`
- `src/tauri/src/mcp/config.rs`
- `src/tauri/src/mcp/wait.rs`
- `src/tauri/src/mcp/tools/mod.rs`
- `src/tauri/src/mcp/tools/types.rs`
- `src/tauri/src/mcp/tools/discovery.rs`
- `src/tauri/src/mcp/tools/lifecycle.rs`
- `src/tauri/src/mcp/tools/logs.rs`
- `src/tauri/src/mcp/tools/git.rs`
- `src/tauri/src/mcp/tools/preview.rs`
- `src/tauri/src/mcp/tools/github.rs`
- `src/tauri/src/mcp/tools/github_pages.rs`
- `src/tauri/src/projects/marker.rs`
- `src/tauri/src/projects/slug.rs`
- `src/tauri/src/auth/github_repos.rs`
- `src/web/components/settings/cursor-integration-panel.tsx` (or path matching existing settings layout)

Modified files:

- `src/tauri/Cargo.toml` (add `rmcp`, `rmcp-macros`, `schemars`, `axum`, possibly `wiremock` for tests)
- `src/tauri/src/app/bootstrap.rs` (spawn MCP server, manage `Arc<GitHubReposClient>`)
- `src/tauri/src/app/register.rs` (register `mcp_status`, `mcp_rotate_token`)
- `src/tauri/src/error.rs` (replace `PublishFailed` with `Publish(PublishError)`, optionally add `GitHubApi` variant)
- `src/tauri/src/projects/types.rs` (add `slug: Option<String>`, `with_slug` builder)
- `src/tauri/src/projects/store.rs` (add `get_by_slug`)
- `src/tauri/src/projects/importer.rs` (post-clone install hook)
- `src/tauri/src/projects/commands.rs` (also wire post-clone install in `clone_repository`)
- `src/tauri/src/infrastructure/git.rs` (`status`, `checkout(create)`, `add_remote`, `current_commit_sha`)
- `src/tauri/src/infrastructure/runtime.rs` (add `tail_logs`)
- `src/tauri/src/vm/runtime.rs` (impl `tail_logs`)
- `src/tauri/src/vm/wsl.rs` (parameterize `logs(lines)`)
- `src/tauri/src/auth/mod.rs` (re-export `GitHubReposClient`)
- `src/tauri/src/publish/deployer.rs`, `builder.rs`, `github_pages.rs` (typed `PublishError` construction)
- `src/web/lib/errors.ts` (new error variants)
- `src/web/lib/errors.test.ts` (coverage for the variants)
- `src/web/gen/tauri.ts` (regenerated)
- `src/web/components/settings/...` (mount the new panel)
- `scripts/check-tauri-command-parity.sh` (add `mcp_status`, `mcp_rotate_token` to expected lists across platforms)

## Open questions deferred to implementation

- Should `.opnble` be auto-committed by Opnble at import (a separate `git commit` after writing the file), or left for the user to commit on their first push? Lean toward "leave for user; commit happens naturally in `github_push`'s `commit_auto`".
- Exact rmcp transport configuration: the `StreamableHttpServerConfig::default()` is `stateful_mode = true`. Confirm during implementation that Cursor handles `Mcp-Session-Id` correctly with our session manager.
- Whether to add a `cancel_operation` MCP tool for in-flight long ops (start_project hung at install). Likely v1.1 once we see real failure modes.

## Out of scope (explicit non-goals)

- Multi-user / shared-machine threat model. Token in `~/.opnble/mcp.json` with `0o600` is sufficient for single-user dev. Per-action approval is v1.1+.
- Auto-discovery of the MCP server by other agent clients (Claude Desktop, ChatGPT). Same JSON works in their config files; doc-only.
- Migration code for projects imported before MCP existed. Alpha = no installed base.
