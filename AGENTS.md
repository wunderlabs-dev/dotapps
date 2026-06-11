# Opnble - Agent instructions

This file is the entrypoint for AI agents working in this repository. **Authoritative policy** lives in `.cursor/rules/*.mdc` (YAML frontmatter plus Markdown bodies). Cursor loads them automatically (`alwaysApply` and `globs`). This document summarizes how to navigate them and how humans run the project.

## Policy layout (Cursor rules)

| File | Scope |
|------|--------|
| `.cursor/rules/opnble-core.mdc` | Always on: product context, engineering principles, architecture, `~/.opnble/` paths |
| `.cursor/rules/opnble-makefile.mdc` | Always on: forbidden raw commands and their `make` equivalents |
| `.cursor/rules/opnble-verification.mdc` | Always on: which gate to run before claiming work is done |
| `.cursor/rules/opnble-canvas.mdc` | Always on: default to Cursor Canvas for visual or interactive output |
| `.cursor/rules/web-frontend.mdc` | When editing `src/web/**/*.{ts,tsx,css}`: full frontend and design-system rules |
| `.cursor/rules/rust-backend.mdc` | When editing `src/tauri/**/*.rs` or `src/agent/**/*.rs`: Clippy policy, Rust style, modules |

Do not recreate legacy agent instruction files: extend the matching `.mdc` rule instead.

## Cursor hooks

`.cursor/hooks.json` registers four hooks (see `.cursor/hooks/` for the scripts):

- `beforeShellExecution` (`makefile-guard.sh`) asks for confirmation when commands like `pnpm dev`, `cargo tauri dev`, `pkill -f tauri`, or direct `vfkit`/`qemu-system` invocations would bypass the Makefile and the `~/.opnble/dev.lock` lockfile.
- `afterFileEdit` (`autoformat.sh`) runs `rustfmt` on `.rs` files and `biome format` on `src/web/**/*.{ts,tsx,js,jsx,json,css}` so format-only diffs do not leak into commits.
- `stop` (`verify-on-stop.sh`) inspects uncommitted code changes and injects a `followup_message` naming the matching gate from `opnble-verification.mdc`.
- `sessionStart` (`session-context.sh`) injects a repo state snapshot (branch, status, last five commits, worktrees, latest plan) at the start of each session.

To extend, add a new event under `hooks` in `hooks.json` and a matching script under `.cursor/hooks/`.

## Slash commands

`.cursor/commands/*.md` files become slash commands. The filename minus the extension is the command name. We use them for the make targets that show up most often:

- `/dev`, `/restart`, `/check`, `/lint-rust`, `/vm-doctor`, `/status` - dev stack control and verification
- `/audit` - run the rules-compliance auditor
- `/pr-review` - interactive PR review through a Canvas

## Subagents

`.cursor/agents/*.md` files become project-scoped subagents callable via the `Task` tool:

- `verifier` - runs the matching gate from `opnble-verification.mdc` and quotes evidence
- `tauri-binding-checker` - read-only specta parity check after Tauri command changes
- `vm-doctor` - runs `make vm-doctor` and maps symptoms to known failure modes
- `architect` - read-only guided tour mode for codebase walkthroughs

## Bugbot

`.cursor/BUGBOT.md` (project root) plus nested `src/web/.cursor/BUGBOT.md`, `src/tauri/.cursor/BUGBOT.md`, and `src/agent/.cursor/BUGBOT.md` encode the rules Bugbot enforces on opened PRs. Enable Bugbot for `vtemian/opnble` in Cursor Settings to activate.

## Cloud Agent Automations

`.cursor/automations.md` is the human-readable source of truth for the PR sentinel and weekly rules-compliance audit automations. Create them in Cursor Settings → Automations using the prompts in that file; the workflow proto schema lives only in Cursor's UI.

## MCP servers

`.cursor/mcp.json` registers `opnble-data`, a filesystem MCP scoped to `~/.opnble/` (state.json, project clones, VM artifacts) via a wrapper script in `.cursor/mcp/`.

## Worktrees

`.cursor/worktrees.json` runs `.cursor/setup-worktree-unix.sh` after `/worktree` creates a worktree: copies `.env`, installs frontend deps, fetches Rust deps, prints the `~/.opnble/dev.lock` constraint.

## Project skills

Reusable project workflows live in `.cursor/skills/`:

- `rust-validate` - Rust fmt, clippy, and tests through Makefile-backed gates
- `tauri-command` - Tauri IPC command creation and registration
- `create-component` - React hook, container, component, and UI primitive scaffolding
- `sharp-edges` - security and footgun audits for Opnble surfaces
- `opnble-rust-guidelines` - Microsoft Pragmatic Rust Guidelines adapted to Opnble's stricter local rules

## Ignore files

`.cursorignore` blocks the agent and Tab from reading or editing `src/web/gen/` (generated specta bindings) and lockfiles. `.cursorindexingignore` trims the semantic index of build outputs, worktrees, vendored deps, and large low-signal files.

## Product

Opnble is a native desktop app (macOS, Linux, Windows) that lets non-technical users open and run React or Next.js projects from git repositories locally. The stack bundles tooling so users do not install Node.js, git, or Docker themselves.

## Repository map

| Area | Path | Role |
|------|------|------|
| Frontend | `src/web/` | React 19, TypeScript, Tailwind 4, Vite 7 |
| Tauri host | `src/tauri/` | Rust backend, IPC commands, VM orchestration, git |
| VM agent | `src/agent/` | Rust gRPC server inside the VM (sync style, no async executor like the host) |

Data flow (simplified): user adds a repo URL → React calls Tauri commands → Rust clones with libgit2 under `~/.opnble/repos/<project-id>/` → container runner starts Node with the repo mounted → logs stream via Tauri events → UI renders in the embedded webview.

## Makefile is the only dev entrypoint

Use **Makefile targets** for install, dev servers, lint, format, tests, and VM lifecycle. Do **not** run raw commands such as `cargo tauri dev`, `pnpm dev`, or `pkill` for project workflows unless a Makefile target wraps them. A lockfile at `~/.opnble/dev.lock` prevents concurrent dev sessions from corrupting VM state.

Common targets:

- `make install` - dependencies
- `make dev-all` - Vite plus Tauri (typical dev)
- `make restart` - kill processes, reset state, start clean
- `make kill-dev` - stop dev processes and clean VM-related dev state
- `make status` - VM, agent, Vite, project status
- `make check` - full quality gate (frontend `pnpm check` plus Rust clippy and `cargo fmt` check)
- `make lint` - format check, frontend lint, Rust clippy
- `make test-all` - Rust tests plus frontend tests

After substantive edits, run the narrowest gate that covers your change; before claiming work is done, run `make check` when both frontend and Rust could be affected.

## CI and automation (what can fail)

- **Frontend** (`.github/workflows/quality-gate.yml`): on pushes and PRs to `main`, when `src/web/` changes, runs `pnpm check` in `src/web` (Biome, ESLint, `tsc --noEmit`, Vitest).
- **Tauri command parity** (`.github/workflows/tauri-command-parity.yml`): ensures registered commands stay consistent with generated or checked artifacts via `scripts/check-tauri-command-parity.sh` when relevant Rust paths change.
- **Worker** (`.github/workflows/worker-deploy.yml`): on pushes and PRs to `main` when `src/worker/` changes, runs `npm run check`; on push to `main`, deploys with `wrangler deploy` using `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` repository secrets.
- **Release** (`.github/workflows/release.yml`, version tags): among other steps, runs web tests, `cargo fmt --check`, `cargo clippy` for the `opnble` package, and Tauri crate tests.

Local `make check` runs Clippy for **both** `opnble` and `opnble-agent`, which is stricter than the release job’s Clippy scope for the host crate alone.

## Cross-cutting expectations

- Prose in docs and UI copy: **do not use em dashes**. Use colons for definitions, commas or parentheses for asides.
- DRY, YAGNI, fail fast, dependency injection, structured errors (no empty catches), name by domain behavior.
- Follow existing naming, file layout, and patterns in the subtree you edit.

## User data paths (not in the repo)

Runtime data lives under `~/.opnble/` (clones, `state.json`, VM images, EFI store). Do not assume these paths exist in the workspace.

## Cursor workspace rules

Cursor should rely on `AGENTS.md` and `.cursor/rules/` for project guidance.
