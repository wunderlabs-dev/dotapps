# VM agent Bugbot rules

Applies to `src/agent/**`. Shares the Rust style rules with the Tauri host ([rust-backend.mdc](../../../.cursor/rules/rust-backend.mdc)) plus these agent-specific constraints.

## No async runtime

The agent is intentionally synchronous: ttrpc + native nix syscalls. Reject any PR that:

- Adds `tokio`, `async-std`, `smol`, or any other async executor as a dependency.
- Introduces `async fn` or `.await` in agent code.
- Imports `futures::executor` or similar.

## Process and signal handling

- All signal handlers go through `nix::sys::signal`, not platform-specific shortcuts.
- Reap child processes via `waitpid` or `wait` immediately after spawn; do not leave zombies.
- File descriptors created by the agent are closed on the same path that opened them, or wrapped in an RAII type.

## gRPC surface

- Every new ttrpc method has a typed request and response defined in the `.proto` schema, regenerated, and committed.
- No ad-hoc JSON over gRPC. Use the generated protobuf types.

## Errors

- Same typed-error rule as the host: structured enums via `thiserror`, no `String` at API boundaries.
- Operating-system errors wrap `nix::Error` or `io::Error` with context, never a bare `unwrap`.
