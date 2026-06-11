#!/usr/bin/env bash
#
# beforeShellExecution: ask the user before running raw dev commands that
# bypass the Makefile (and the ~/.opnble/dev.lock state-safety lockfile).
#
# Allows everything that is read-only (tests, lint, format, install) and
# everything that goes through `make`. Asks for explicit confirmation only
# on commands that actually start dev servers, build artifacts, or kill
# processes that the Makefile already manages.
#
# Stdin: JSON with at least { "command": "<full shell command string>" }
# Stdout: JSON with one of:
#   { "permission": "allow" }
#   { "permission": "ask",  "agent_message": "...", "user_message": "..." }
#
# Per Cursor docs: exit 0 with `permission: "allow"` is the silent passthrough.
# Any other behavior than printing JSON falls open by default.

set -uo pipefail

input=$(cat)
command=$(printf '%s' "$input" | jq -r '.command // empty' 2>/dev/null || true)

allow() { printf '{"permission":"allow"}\n'; exit 0; }
ask() {
  local agent="$1" user="$2"
  jq -n --arg a "$agent" --arg u "$user" \
    '{permission:"ask", agent_message:$a, user_message:$u}'
  exit 0
}

[[ -z "$command" ]] && allow

# Strip leading whitespace, quoted strings, and `env VAR=...` prefixes for matching.
normalized=$(printf '%s' "$command" | tr -s ' \t\n')

# 1) Anything wrapped in `make ` is fine.
if [[ "$normalized" =~ (^|[\;\&\|]\ *)make\  ]]; then
  allow
fi

# 2) Read-only frontend scripts (whitelist explicit subcommands).
if [[ "$normalized" =~ pnpm\ +(check|lint|test|format|install|i|typecheck|exec|biome|run\ +(check|lint|test|format|typecheck))($|\ |\;|\&|\|) ]]; then
  allow
fi

# 3) Read-only / safe cargo subcommands.
if [[ "$normalized" =~ cargo\ +(check|clippy|fmt|test|build\ +-p|metadata|tree|doc)($|\ |\;|\&|\|) ]]; then
  allow
fi

# --- Patterns we want to ask about ---

# pnpm dev variants
if [[ "$normalized" =~ pnpm\ +(run\ +)?dev($|\ |\;|\&|\|) ]]; then
  ask \
    "Use 'make dev-frontend' (Vite alone) or 'make dev-all' (Vite + Tauri) instead of raw 'pnpm dev'. The Makefile holds ~/.opnble/dev.lock to prevent concurrent VM corruption." \
    "This command starts the Vite dev server outside the Makefile, which can corrupt VM state. Prefer 'make dev-all'."
fi

# cargo tauri dev / build
if [[ "$normalized" =~ cargo\ +tauri\ +(dev|build)($|\ |\;|\&|\|) ]]; then
  ask \
    "Use 'make dev' (or 'make dev-all') for development and 'make build' for production. Calling 'cargo tauri' directly skips the dev lockfile and the prepare-resources step." \
    "Calling 'cargo tauri ${BASH_REMATCH[1]}' directly bypasses Make scaffolding (lockfile, resource prep). Use the Make target instead."
fi

# pkill on processes the Makefile owns
if [[ "$normalized" =~ pkill ]] && [[ "$normalized" =~ (tauri|vite|vfkit|qemu|cloudflared|opnble) ]]; then
  ask \
    "Use 'make kill-dev' to stop the dev stack cleanly (kills processes AND clears VM state). 'pkill' alone leaves ~/.opnble/dev.lock and partial VM clones behind." \
    "'pkill' on a Make-managed process can leave stale VM state and a held dev lockfile. Prefer 'make kill-dev'."
fi

# Direct VM tooling outside Make (rare, but worth flagging)
if [[ "$normalized" =~ (^|[\;\&\|]\ *)(vfkit|qemu-system) ]]; then
  ask \
    "VM lifecycle is owned by the Makefile and src/tauri/src/vm/. Run 'make dev-all' or 'make vm-image' instead of invoking the hypervisor directly." \
    "Direct hypervisor invocation can leave the EFI store and runtime image in an inconsistent state. Use the Make target."
fi

allow
