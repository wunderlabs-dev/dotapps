#!/bin/bash
# afterFileEdit hook: runs the right formatter for the edited file.
# Fail-open: errors never block the agent, they just skip formatting.

set -u

input=$(cat)
file_path=$(printf '%s' "$input" | jq -r '.file_path // .tool_input.file_path // .tool_input.path // empty' 2>/dev/null)

if [[ -z "$file_path" || ! -f "$file_path" ]]; then
  exit 0
fi

case "$file_path" in
  *.rs)
    if command -v rustfmt >/dev/null 2>&1; then
      rustfmt --edition 2021 "$file_path" >/dev/null 2>&1 || true
    fi
    ;;
  src/web/*.ts|src/web/*.tsx|src/web/*.js|src/web/*.jsx|src/web/*.json|src/web/*.css)
    if [[ -x src/web/node_modules/.bin/biome ]]; then
      ( cd src/web && node_modules/.bin/biome format --write "../../$file_path" >/dev/null 2>&1 ) || true
    fi
    ;;
esac

exit 0
