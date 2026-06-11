---
name: tauri-command
description: Use when adding or changing Opnble Tauri IPC commands, specta bindings, command registration, AppError variants, or frontend command calls - keeps Rust handlers, generated TypeScript, and frontend error translation aligned.
---

# Tauri Command

Tauri commands are Opnble's IPC contract. Treat a command change as a cross-stack API change.

## Placement

Use the existing module unless the command clearly belongs elsewhere:

- `src/tauri/src/projects/commands.rs`: project CRUD, git, runner operations.
- `src/tauri/src/auth/commands.rs`: authentication and provider discovery.
- `src/tauri/src/settings/commands.rs`: app settings.
- `src/tauri/src/tunnel/commands.rs`: sharing.
- `src/tauri/src/app/window_commands.rs`: window management.
- `src/tauri/src/app/platform/**`: platform or VM commands.

Ask before creating a new command module.

## Rust Handler Rules

- Every IPC handler has both `#[tauri::command]` and `#[specta::specta]`.
- Return `Result<T, AppError>`.
- Use `State<'_, Arc<dyn Trait>>` for injected state in async commands.
- Tauri-owned deserialized params may need `#[expect(clippy::needless_pass_by_value, reason = "Tauri command handler receives owned deserialized values")]`.
- Camel-case frontend params may need `#[allow(non_snake_case, reason = "Tauri deserializes camelCase from JS frontend")]`.
- Response DTOs derive `Debug`, `Serialize`, and `specta::Type`, with `#[serde(rename_all = "camelCase")]`.

## Registration

Add the command to `tauri_specta::collect_commands!` in `src/tauri/src/app/register.rs`.

If the command is platform-specific, add the native implementation and matching stubs so every `run_*` path has a complete command set.

## Frontend Usage

Import from `@/gen/tauri`, then bridge through the local Result helper. Follow the current frontend rule:

```ts
const result = await fromTauriResult(commands.commandName(args));
return result.match(onSuccess, onError);
```

Do not use raw `try/catch` or generated-command `unwrap()` in hooks.

## Error Translation

If a new `AppError` variant can reach the UI, update `src/web/lib/errors.ts` with a user-facing title and message.

## Verification

Run the narrowest gate that covers the change:

- Rust command only: `make lint-rust` then `make test`.
- Rust plus frontend command use: `make check` then `make test-all`.
- Command registration parity concern: `bash scripts/check-tauri-command-parity.sh`.

Report the command output evidence before claiming completion.
