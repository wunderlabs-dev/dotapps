#!/bin/bash
# stop hook: when the agent finishes a turn with uncommitted code changes,
# inject a follow-up that names the right verification gate per
# .cursor/rules/opnble-verification.mdc.
# Fail-open: errors never block.

set -u

cat > /dev/null 2>&1

files=$( { git diff --name-only HEAD 2>/dev/null; git diff --name-only --cached 2>/dev/null; } | sort -u )

if [[ -z "$files" ]]; then
  echo '{}'
  exit 0
fi

has_rust=false
has_web=false
has_other_code=false

while IFS= read -r f; do
  [[ -z "$f" ]] && continue
  case "$f" in
    *.md|*.mdc|.cursor/*|.github/*|Makefile|scripts/*|docs/*|*.toml|.gitignore|.cursorignore|.cursorindexingignore|AGENTS.md|README.md|CHANGELOG.md)
      ;;
    src/tauri/*.rs|src/agent/*.rs)
      has_rust=true ;;
    src/web/*.ts|src/web/*.tsx|src/web/*.js|src/web/*.jsx|src/web/*.css)
      has_web=true ;;
    *.rs)
      has_rust=true ;;
    *)
      case "$f" in
        src/tauri/*|src/agent/*) has_rust=true ;;
        src/web/*) has_web=true ;;
        *) has_other_code=true ;;
      esac
      ;;
  esac
done <<< "$files"

if [[ "$has_rust" == "false" && "$has_web" == "false" && "$has_other_code" == "false" ]]; then
  echo '{}'
  exit 0
fi

if [[ "$has_rust" == "true" && "$has_web" == "true" ]]; then
  gate="make check && make test-all"
elif [[ "$has_rust" == "true" ]]; then
  gate="make lint-rust && make test"
elif [[ "$has_web" == "true" ]]; then
  gate="cd src/web && pnpm check"
else
  gate="make check"
fi

msg="Before declaring this done, verify per .cursor/rules/opnble-verification.mdc: run \`$gate\` and quote the actual pass/fail line. If you already ran the gate this turn and quoted the output, ignore this nudge."

if command -v jq >/dev/null 2>&1; then
  jq -n --arg m "$msg" '{followup_message: $m}'
else
  printf '{"followup_message": %s}\n' "$(printf '%s' "$msg" | sed 's/\\/\\\\/g; s/"/\\"/g' | awk 'BEGIN{printf "\""} {printf "%s", $0} END{printf "\""}')"
fi

exit 0
