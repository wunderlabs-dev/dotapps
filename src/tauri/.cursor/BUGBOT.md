# Tauri host Bugbot rules

Applies to `src/tauri/**`. Authoritative rules: [rust-backend.mdc](../../../.cursor/rules/rust-backend.mdc).

## Tauri commands

- Every `#[tauri::command]` is registered in `tauri::Builder::invoke_handler`.
- Every command returns `Result<T, AppError>`, never `Result<T, String>` or a bare type.
- Specta bindings (`src/web/gen/tauri.ts`) must be regenerated if any command surface changes.
- Use the `tauri-command` skill at `.cursor/skills/tauri-command/SKILL.md` when adding a command.

## Error handling

- No `.unwrap()` or `.expect()` in production code paths. The only exceptions are: test code, `OnceLock::set` where the lock is genuinely infallible by construction, and `Mutex::lock()` on a known non-poisoned mutex with a comment explaining the invariant.
- Errors are typed `thiserror`-derived enums, not `String` or `anyhow::Error` at API boundaries.
- Use `From<SourceError> for AppError` impls instead of repeated `.map_err(|e| ...)` closures.

## Concurrency

- No `tokio::spawn` without a `JoinHandle` stored in a struct or attached to a task tracker. No fire-and-forget tasks.
- Shared state goes through `Arc<Mutex<T>>` or `tauri::State<T>`, not `static` `OnceLock` containers.
- Do not hold a `MutexGuard` across an `.await` point. Drop the guard explicitly before awaiting.

## Naming

- No `*Manager`, `*Handler`, `*Service`, `*Helper`, `*Util`, or `*Utils` types. Name by what the type does in the domain.
- Functions: verb phrases. Types: noun phrases.

## VM and process lifecycle

- Any new process spawn must be reaped: `Child::wait()` or `tokio::process::Child::wait()` somewhere in the lifecycle. No detached processes.
- Any new port-forward or tunnel must have a registered shutdown path called from `app/shutdown.rs`.

## Modules

- One responsibility per module. If `mod.rs` grows past ~400 lines, split it.
- Public API at the top of the file. Private helpers at the bottom.
