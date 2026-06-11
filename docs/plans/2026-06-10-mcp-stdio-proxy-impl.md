# MCP stdio proxy: replace url+headers Cursor entries with a command shim

> Implementation plan for an executing agent. Work phase by phase, TDD where a test seam exists, run the verification gate at the end of each phase before moving on, one commit per phase (or per task where marked). Quote gate output as evidence per `opnble-verification.mdc`.

**Goal:** Cursor must never cache Opnble's MCP server as "errored" when the app is down. Replace the `url` + `headers` entry Opnble writes into `~/.cursor/mcp.json` and `<repo>/.cursor/mcp.json` with a `command` (stdio) entry that launches `opnble mcp-proxy`: a headless bridge that answers the MCP handshake locally, forwards tools traffic to the local HTTP listener at `http://127.0.0.1:47821/mcp`, retries with backoff while the app is down, and reconnects transparently when the app restarts.

**Architecture:** subcommand of the existing `opnble` binary (no new `[[bin]]`, no sidecar packaging). `main.rs` branches on `argv[1] == "mcp-proxy"` before any Tauri code runs. The proxy is a tools-only MCP server on stdio (rmcp `ServerHandler`) holding a supervised rmcp client `Peer<RoleClient>` over `StreamableHttpClientTransport` to the in-app listener.

**Tech stack:** rmcp 1.7 (add features `client`, `transport-streamable-http-client-reqwest`, `transport-io`), tokio, existing `~/.opnble/mcp.json` as the token + url source of truth. No new dependencies (no clap: one subcommand, one optional flag).

**Companion context:** root-cause analysis in this chat (Cursor connects MCP at window load, caches connection-refused as errored for the whole session, and a byte-identical config rewrite triggers no reconnect). Research findings inlined below where they drive a decision.

---

## Design decisions (locked)

1. **Subcommand, not sidecar.** The app has no single-instance plugin and `main()` is a one-liner into `opnble_lib::run()`, so branching on argv is safe and costs zero packaging work. A separate `[[bin]]` would require `externalBin` bundler config plus build-order changes for four platforms. The installer writes `std::env::current_exe()` as the command path; configs self-heal on every app bootstrap and every open-in-Cursor because the entry is rewritten there.
2. **Token leaves the per-project files.** The written entry becomes `{ "command": <exe>, "args": ["mcp-proxy", "--mcp-config", <abs path to ~/.opnble/mcp.json>] }`. The proxy reads `{ url, token }` from that file on every connect attempt, so token rotation needs no Cursor-config sweep to keep working, and project `.cursor/mcp.json` files no longer carry a secret. Keep writing the `.gitignore` line anyway (the file still encodes a machine-local absolute path nobody should commit).
3. **Explicit `--mcp-config` path, resolved at install time.** Cursor spawns stdio servers with an isolated environment; do not rely on `$HOME` in the child. `dirs::home_dir()` would still work via getpwuid, but the explicit arg makes the contract visible and testable. Fallback when the flag is absent: `paths::base_dir()?.join("mcp.json")`.
4. **Initialize is answered locally, never blocked on upstream.** rmcp's server machinery answers `initialize` from `ServerHandler::get_info()`. Static capabilities: tools only, `listChanged: true`, mirroring `OpnbleMcp`. Blocking initialize would burn Cursor's ~60 s handshake window and recreate the bug.
5. **Degraded-mode semantics.** `tools/list` with upstream down: wait up to 8 s, then return an empty list (not an error). `tools/call` with upstream down: wait up to 15 s, then JSON-RPC error "Opnble is not running; start the app and retry". When upstream (re)connects, emit `notifications/tools/list_changed` so Cursor re-fetches. This is conformant with MCP 2025-03-26.
6. **Two-tier reconnect.** Session-expired (HTTP 404, app alive): handled entirely by rmcp via `reinit_on_expired_session(true)`. Process-down (connection refused, SSE dead, or 401 after rotation): a supervisor loop re-reads the token file, rebuilds the transport, re-handshakes upstream (fresh session id), and notifies downstream. Backoff: 500 ms doubling to a 5 s cap, plus or minus 20% jitter, retry forever, reset on success.
7. **Exit on stdin EOF only.** Cursor has known lifecycle bugs (orphaned children on quit, no SIGTERM on window reload). The proxy exits when `serve_server(...).waiting()` returns (stdin closed) and never exits on upstream failure.
8. **stdout is sacred.** Only JSON-RPC on stdout. Proxy `tracing` goes to stderr (Cursor shows it in the per-server MCP log). The workspace `print_stdout`/`print_stderr` deny lints already force this through `tracing`.
9. **Token rotation keeps its sweep, simplified in meaning.** `mcp_rotate_token` still calls `sweep_token` so stale url-shaped entries anywhere get migrated to the command shape, but the rewrite is now token-independent and idempotent. `RotateReport` semantics ("N updated, M failed") stay valid; the frontend contract does not change.
10. **Startup migration.** Bootstrap already writes the global entry; add a best-effort sweep over all imported projects so existing url-shaped `.cursor/mcp.json` files migrate to the command shape on first launch of the new build, not just on next import/open/fork.

## Engineer assumptions

- You can build and test with `make lint-rust`, `cd src/tauri && cargo test`, `make check`. Do not run raw `cargo tauri dev`; use `make dev-all` if a live app is needed.
- Clippy pedantic is deny-by-default: no `unwrap`, no `as` casts, no indexing, `#[expect(..., reason)]` for any suppression. `mod.rs` files hold only declarations and re-exports.
- The specta command surface does not change in this plan (no new Tauri commands), so `scripts/check-tauri-command-parity.sh` needs no edits.
- Generated bindings `src/web/gen/` are cursorignored; the `McpStatus` shape does not change, so no binding regen is strictly required.

---

# Phase 1: proxy entrypoint plumbing

**Outcome:** `opnble mcp-proxy --mcp-config <path>` starts a tokio runtime, logs to stderr, and exits cleanly on stdin EOF. No forwarding yet.

1. **Cargo features.** In `src/tauri/Cargo.toml` extend rmcp:

   ```toml
   rmcp = { version = "1.7", features = [
     "server", "macros", "transport-streamable-http-server",
     "client", "transport-streamable-http-client-reqwest", "transport-io",
   ] }
   ```

   Run `make lint-rust` immediately: the added client stack must not trip workspace lints in existing code.

2. **Argv branch.** `src/tauri/src/main.rs`:

   ```rust
   fn main() {
       let mut args = std::env::args().skip(1);
       if args.next().as_deref() == Some("mcp-proxy") {
           if let Err(e) = opnble_lib::mcp::proxy::run(args) {
               tracing::error!(error = %e, "mcp proxy failed");
               std::process::exit(1);
           }
           return;
       }
       if let Err(e) = opnble_lib::run() {
           tracing::error!(error = %e, "bootstrap failed");
           std::process::exit(1);
       }
   }
   ```

   Unknown argv values fall through to the GUI exactly as today (macOS passes `-psn_...` flags on Finder launches; do not treat unknown args as errors).

3. **Module skeleton.** New `src/tauri/src/mcp/proxy/` with `mod.rs` (declarations + `pub use self::runtime::run`), `runtime.rs`, `handler.rs`, `upstream.rs`. `runtime.rs` owns:
   - flag parsing: accept `--mcp-config <path>`; anything else is an error with a usage message via `AppError::Internal`
   - its own `tracing_subscriber` writing to stderr (do not reuse `app::logging::init`, which may target files or stdout)
   - `tokio::runtime::Runtime::new()` then `block_on(serve(...))`
4. **Read-only token loading.** In `mcp/auth.rs` add `pub fn read_token_file(path: &Path) -> Result<McpFile, AppError>` (make `McpFile` and its fields `pub(crate)`), strictly read-only: a missing or invalid file is an `Err`, never a rotate. The proxy is not the owner of that file; only the app generates tokens. The proxy treats `Err` like "upstream not ready" and retries.
5. **Tests (TDD where possible).**
   - `proxy::runtime` flag parsing: valid flag, missing value, unknown flag.
   - `auth::read_token_file`: missing file errors (does not create), invalid token errors, valid file round-trips.

**Gate:** `make lint-rust && (cd src/tauri && cargo test mcp::)`. Commit: `feat(mcp): add opnble mcp-proxy entrypoint and read-only token loading`.

# Phase 2: proxy core (handler, supervisor, reconnect)

**Outcome:** a working bridge. `handler.rs`:

```rust
#[derive(Clone)]
pub struct ProxyHandler {
    upstream: tokio::sync::watch::Receiver<Option<Peer<RoleClient>>>,
}

impl ServerHandler for ProxyHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo { capabilities: ServerCapabilities::builder().enable_tools().build(), ..Default::default() }
    }
    // list_tools: wait_for_upstream(8s) -> forward, else Ok(ListToolsResult::default())
    // call_tool:  wait_for_upstream(15s) -> forward, else ErrorData::internal_error("Opnble is not running; start the app and retry", None)
}
```

`upstream.rs`:

1. **`Backoff`** struct: 500 ms start, doubling, 5 s cap, 20% jitter (use a tiny xorshift or `getrandom`, no new dep), `reset()`. Pure and unit-tested.
2. **`ForwardingClientHandler`** implementing rmcp `ClientHandler`: `on_tool_list_changed` forwards `notify_tool_list_changed()` to the downstream `Peer<RoleServer>` (capture it via `RequestContext` on first downstream request, stored in `Arc<OnceLock<_>>`, or pass the running server's peer in after `serve_server` returns: prefer the latter, it is deterministic).
3. **Supervisor loop:** per attempt, `auth::read_token_file(config_path)`, build `StreamableHttpClientTransportConfig::with_uri(file.url).auth_header(file.token).reinit_on_expired_session(true)`, `serve_client`. On success: publish peer to the `watch` channel, reset backoff, emit `tools/list_changed` downstream, then `running.waiting().await`; when it returns, publish `None` and loop. On connect error: sleep `backoff.next()`.
   - Upstream URL comes from the token file (`McpFile.url`) with fallback to `config::mcp_url()`, so a future port change migrates without touching written Cursor configs.
   - A 401 connect failure (token rotated between reads) is just another retry; the next loop iteration re-reads the file and picks up the new token.
4. **`runtime::serve`:** spawn supervisor, `serve_server(handler, rmcp::transport::io::stdio())`, hand the downstream peer to the supervisor, `running.waiting().await`, exit 0. No signal handling.
5. **Per-request timeouts.** Set explicit timeouts on forwarded requests (`PeerRequestOptions`) so a hung upstream cannot wedge the stdio session: 30 s for `call_tool` (exec_command can be slow; pick generously), 10 s for `list_tools`.
6. **Tests.**
   - `Backoff`: cap, reset, jitter bounds.
   - Handler degraded mode: with `watch` holding `None`, `list_tools` returns empty within budget, `call_tool` returns the error message (drive with short test budgets via injected durations, not 8/15 s sleeps).
   - Integration (the valuable one): in-process axum server from `mcp::server::build_router` with a known token + `tokio::io::duplex` as the stdio pair. Assert: (a) client over the duplex completes initialize while upstream is down; (b) after the upstream binds, `tools/list_changed` arrives and `tools/list` returns the real tool set; (c) kill the upstream task, restart it, and a subsequent `call_tool` succeeds after reconnect.
   - Token rotation: rewrite the temp token file mid-test, kill the upstream session, assert the supervisor reconnects with the new token (old token now 401s).

**Gate:** `make lint-rust && (cd src/tauri && cargo test mcp::proxy)`. Commit: `feat(mcp): stdio proxy with supervised upstream reconnect`.

# Phase 3: installer writes the command shape

**Outcome:** every writer produces the new entry; existing url-shaped entries migrate.

1. **Entry shape.** In `install.rs` replace `opnble_entry(token, port)` with:

   ```rust
   fn opnble_entry(exe: &Path, mcp_config: &Path) -> Value {
       json!({
           "command": exe.to_string_lossy(),
           "args": ["mcp-proxy", "--mcp-config", mcp_config.to_string_lossy()],
       })
   }
   ```

   `serde_json::Map::insert` replaces the whole entry, so stale `url`/`headers` keys vanish on rewrite: migration is free at every write site.

2. **Resolve paths once.** Add a small `InstallTarget { exe: PathBuf, mcp_config: PathBuf }` resolved via `std::env::current_exe()?` and `auth::config_path()?` (drop its `#[allow(dead_code)]`). Thread it through `write_mcp_config_at`, `write_project_mcp_config`, `write_global_mcp_config`, `sweep_token`, replacing the `token: &str, port: u16` params. `install_project_entry` keeps its `(repo_path, label)` signature and resolves `InstallTarget` internally; it no longer needs `auth::current_token()` (a missing token file is no longer a reason to skip the install).
3. **Call sites.** Update the four writers found in exploration:
   - `app/bootstrap.rs::install_global_cursor_mcp` (global entry)
   - `projects/importer.rs::after_clone_install` (unchanged call, new internals)
   - `projects/cursor.rs::backfill_project_mcp` (unchanged call)
   - `mcp/tools/discovery.rs::fork_project_impl` (switch from `current_token()` + `write_project_mcp_config` to `install_project_entry` or the new signature)
4. **Startup migration sweep.** In bootstrap, after the global install, run a best-effort `install::sweep_configs(store)` (extracted from `sweep_token_with_global`, minus rotation) over all imported projects so existing repos migrate without an import/open/fork event. Log a summary line.
5. **Rotation.** `auth::rotate_token` is unchanged. `mcp_rotate_token` still calls the sweep (now shape-only); `RotateReport` unchanged.
6. **Tests.** Rewrite `install.rs` tests for the command shape (assert `command` ends with the test exe path, `args[0] == "mcp-proxy"`, `args[2]` is the injected config path, no `headers` key survives a rewrite over a seeded url-shaped entry). Keep the merge-preservation, invalid-JSON, gitignore, and sweep tests; they are shape-agnostic except for assertions.

**Gate:** `make lint-rust && (cd src/tauri && cargo test)` (full crate: importer/cursor/discovery tests touch this). Commit: `feat(mcp): install command-shaped Cursor entries pointing at opnble mcp-proxy`.

# Phase 4: frontend copy and docs

**Outcome:** the Settings panel stops claiming the token lives in project files.

1. `src/web/components/settings-panel/cursor-integration-body.tsx` (lines 76 to 94): update the config-locations help text. New truth: `~/.opnble/mcp.json` holds the url and bearer token (app-managed); each project's `.cursor/mcp.json` holds a command entry that launches the Opnble MCP proxy; rotating the token takes effect on the proxy's next reconnect without rewriting project files. Keep the `status.url` display (still the real endpoint) and the rotate button.
2. `cursor-integration-section.test.tsx`: fixtures unchanged (`McpStatus` shape is untouched); update any copy assertions that mention the old text.
3. Docs: this plan is the record. Optionally append a short "superseded by" note to the `.cursor/mcp.json` shape section of `docs/plans/2026-05-14-local-agentic-dev-env-design.md` (lines 99 to 114) pointing here. Do not rewrite history in that doc.

**Gate:** `cd src/web && pnpm check`. Commit: `docs(settings): describe command-based Cursor MCP install`.

# Phase 5: end-to-end verification (manual, with evidence)

Run with the real Cursor on this machine:

1. `make restart`, wait for the app, confirm `~/.cursor/mcp.json` and `~/.opnble/repos/proj-1777990751635/.cursor/mcp.json` now hold the command shape.
2. Quit the Opnble app entirely. Reload the Cursor window on the agentic-conference project. Expected: MCP server `opnble` shows connected with 0 tools (not errored).
3. Start the app (`make dev-all`). Expected within ~5 s: tool count goes to 21 without touching Cursor settings. Run `create_preview_link` from a chat.
4. `make restart` while a Cursor chat is open. Expected: a tool call during the down window returns the "Opnble is not running" error; after restart the next call succeeds with no manual reload.
5. Rotate the token from Settings. Expected: next tool call still succeeds (proxy reconnects with the new token).
6. Quit Cursor; verify no orphaned `opnble mcp-proxy` processes remain (`pgrep -lf "mcp-proxy"`).

Record each step's outcome in the PR description. Final gate: `make check && make test-all`.

---

## Risks and watch items

| Risk | Mitigation |
|---|---|
| Cursor ignores `tools/list_changed` (known MCP client FSM races in recent Cursor builds) | Verified empirically in Phase 5 step 3; if broken, fallback is documented guidance to reload, which is still strictly better than the errored-cache status quo |
| rmcp client features bloat the app binary or trip pedantic lints in generated code | Checked at Phase 1 step 1 before any real work |
| Cancellation not bridged: a cancelled `tools/call` keeps running upstream | Accepted for v1; `exec_command` already has server-side timeouts |
| Stale exe path after the app moves or updates | Every bootstrap rewrites global + sweeps projects with the current `current_exe()` |
| Multiple Cursor windows spawn multiple proxies | Each gets its own upstream session; `LocalSessionManager` supports concurrent sessions; verify in Phase 5 |
| Dev/prod builds fight over the config | Last app to start wins the exe path; identical to today's behavior with the token, and self-healing |
