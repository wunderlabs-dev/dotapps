#!/bin/bash
# sessionStart hook: injects "where we left off" context so the agent
# doesn't ask the user to re-bootstrap the plan / branch / WIP at the
# start of every session.
# Fail-open: errors never block.

set -u

cat > /dev/null 2>&1

branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
status_short=$(git status --short 2>/dev/null | head -20)
recent_commits=$(git log --oneline -5 2>/dev/null)

latest_plan=""
if [[ -d docs/plans ]]; then
  latest_plan=$(ls -t docs/plans/*.md 2>/dev/null | head -1)
fi

worktree_note=""
worktrees=$(git worktree list 2>/dev/null | wc -l | tr -d ' ')
if [[ "${worktrees:-1}" -gt 1 ]]; then
  worktree_note=$(git worktree list 2>/dev/null)
fi

context=""
context+="Repo state snapshot for this session (from .cursor/hooks/session-context.sh):"$'\n\n'
context+="Branch: $branch"$'\n'

if [[ -n "$status_short" ]]; then
  context+=$'\n'"Uncommitted changes:"$'\n'"$status_short"$'\n'
else
  context+=$'\n'"Working tree: clean"$'\n'
fi

if [[ -n "$recent_commits" ]]; then
  context+=$'\n'"Recent commits:"$'\n'"$recent_commits"$'\n'
fi

if [[ -n "$worktree_note" ]]; then
  context+=$'\n'"Active worktrees:"$'\n'"$worktree_note"$'\n'
fi

if [[ -n "$latest_plan" ]]; then
  plan_title=$(head -3 "$latest_plan" | grep -m1 -E '^#' || basename "$latest_plan")
  context+=$'\n'"Latest plan: $latest_plan"$'\n'"  $plan_title"$'\n'
fi

context+=$'\n'"Use this only as orientation. If anything looks stale, re-check with git status."

if command -v jq >/dev/null 2>&1; then
  jq -n --arg c "$context" '{additional_context: $c}'
else
  printf '{"additional_context": %s}\n' "$(printf '%s' "$context" | python3 -c 'import json,sys;print(json.dumps(sys.stdin.read()))' 2>/dev/null || printf '"%s"' "$(printf '%s' "$context" | sed 's/\\/\\\\/g; s/"/\\"/g; s/$/\\n/' | tr -d '\n')")"
fi

exit 0
