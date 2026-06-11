# Opnble Bugbot rules (project root)

These are the always-apply review constraints. Subtree-specific rules live in `src/web/.cursor/BUGBOT.md`, `src/tauri/.cursor/BUGBOT.md`, and `src/agent/.cursor/BUGBOT.md`. The authoritative project rules are in [.cursor/rules/](../.cursor/rules), see:

- [opnble-core.mdc](../.cursor/rules/opnble-core.mdc)
- [opnble-makefile.mdc](../.cursor/rules/opnble-makefile.mdc)
- [opnble-verification.mdc](../.cursor/rules/opnble-verification.mdc)
- [opnble-canvas.mdc](../.cursor/rules/opnble-canvas.mdc)

Bugbot must flag any PR that violates the following.

## Writing

- No em-dashes anywhere (commit messages, prose, code comments, docstrings, UI copy). Use colons for definitions, commas or parentheses for asides.

## Engineering

- No `*Manager` type names anywhere (Rust, TypeScript). Use a name that describes what the type does in the domain.
- No `*Ids: Set<...>` pending-state pattern in the frontend. Use TanStack Query's mutation state instead. See [.cursor/rules/web-frontend.mdc](../.cursor/rules/web-frontend.mdc).
- No copy-pasted `map_err` closures. If the same closure appears more than once, implement `From<SourceError> for TargetError` instead.
- No global mutable state (`static mut`, `OnceLock`, `lazy_static`) holding anything other than truly immutable config.
- Errors are values, not strings. Use structured `AppError` variants with context. No bare `catch {}` blocks.

## Comments

Bias toward zero inline comments. Comments are a last resort. Reject comments that:

- Narrate what the next line does ("// increment counter").
- Restate file or function structure ("// 1. Validate", "// 2. Process").
- Describe a flow the type signature already shows.
- Reference temporal state ("TODO later", "in a future PR", "fixed the bug").
- Apologize or hedge ("this is hacky", "not sure if this is right").

Comments are allowed only for non-obvious *why*, a load-bearing invariant the type system cannot express, or a cross-reference to an external bug/spec/issue.

## Tooling

- No raw `pnpm dev`, `cargo tauri dev`, `cargo tauri build`, `pkill -f tauri`, `pkill -f vfkit`, or direct hypervisor calls. Use the matching `make` target. See [.cursor/rules/opnble-makefile.mdc](../.cursor/rules/opnble-makefile.mdc).
- No bypassing pre-commit or pre-push hooks (`--no-verify`, `--no-gpg-sign`) without an explicit justification in the PR description.

## Verification

- Any PR that touches `src/web/**/*.{ts,tsx,css}` must pass `pnpm check` (Biome + ESLint + tsc + Vitest).
- Any PR that touches `src/tauri/**/*.rs` or `src/agent/**/*.rs` must pass `make lint-rust` and `make test`.
- Any PR that touches both stacks must pass `make check` and `make test-all`.
- Generated files in `src/web/gen/` must not be edited by hand; they regenerate from Rust commands.
