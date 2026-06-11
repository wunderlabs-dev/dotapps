#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SKILLS_DIR="$ROOT_DIR/.cursor/skills"

if [[ ! -d "$SKILLS_DIR" ]]; then
  echo "[skills] no .cursor/skills directory"
  exit 0
fi

failed=0

while IFS= read -r -d '' skill_file; do
  rel_path="${skill_file#"$ROOT_DIR"/}"
  skill_dir="$(basename "$(dirname "$skill_file")")"

  if ! awk 'NR == 1 && $0 == "---" { found = 1 } END { exit found ? 0 : 1 }' "$skill_file"; then
    echo "[skills] $rel_path: missing opening frontmatter delimiter" >&2
    failed=1
  fi

  if ! awk 'NR > 1 && $0 == "---" { found = 1; exit } END { exit found ? 0 : 1 }' "$skill_file"; then
    echo "[skills] $rel_path: missing closing frontmatter delimiter" >&2
    failed=1
  fi

  name="$(awk -F': ' '/^name: / { print $2; exit }' "$skill_file")"
  description="$(awk -F': ' '/^description: / { print $2; exit }' "$skill_file")"

  if [[ -z "$name" ]]; then
    echo "[skills] $rel_path: missing name" >&2
    failed=1
  elif [[ "$name" != "$skill_dir" ]]; then
    echo "[skills] $rel_path: name '$name' must match directory '$skill_dir'" >&2
    failed=1
  fi

  if [[ ! "$name" =~ ^[a-z0-9-]+$ ]]; then
    echo "[skills] $rel_path: name must be lowercase letters, digits, and hyphens" >&2
    failed=1
  fi

  if [[ -z "$description" ]]; then
    echo "[skills] $rel_path: missing description" >&2
    failed=1
  fi

done < <(find "$SKILLS_DIR" -mindepth 2 -maxdepth 2 -name SKILL.md -print0 | sort -z)

if [[ "$failed" -ne 0 ]]; then
  echo "[skills] FAILED" >&2
  exit 1
fi

echo "[skills] OK"
