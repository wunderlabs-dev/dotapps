#!/bin/bash
# Runs after Cursor creates a new git worktree. Prepares the worktree
# so an agent can work in it without first having to install deps.
# See https://cursor.com/docs/configuration/worktrees and
# .cursor/rules/opnble-makefile.mdc for the dev-lock constraint.

set -euo pipefail

ROOT="${ROOT_WORKTREE_PATH:-}"
HERE="$(pwd)"

echo "[opnble-worktree] setup in $HERE (root: $ROOT)"

if [[ -n "$ROOT" && -f "$ROOT/.env" && ! -f .env ]]; then
  cp "$ROOT/.env" .env
  echo "[opnble-worktree] copied .env from root"
fi

if [[ -d src/web ]]; then
  echo "[opnble-worktree] installing frontend deps"
  ( cd src/web && pnpm install --frozen-lockfile ) || ( cd src/web && pnpm install )
fi

if [[ -f Cargo.toml ]]; then
  echo "[opnble-worktree] fetching Rust deps"
  cargo fetch
fi

cat <<'NOTE'

[opnble-worktree] Setup done.

Reminder: only one Opnble dev stack can run at a time across all worktrees,
gated by ~/.opnble/dev.lock. Do not run `make dev-all` here if a session is
already running elsewhere. Use `make status` to check, or `make kill-dev` in
the other worktree first.
NOTE
