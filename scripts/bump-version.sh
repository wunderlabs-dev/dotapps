#!/usr/bin/env bash
#
# bump-version.sh <semver>
#
# Updates every place a version string is tracked, in lockstep:
#   - src/tauri/tauri.conf.json     (canonical Tauri app version)
#   - src/tauri/Cargo.toml          (host crate)
#   - src/agent/Cargo.toml          (VM agent crate)
#   - src/web/package.json          (frontend display)
#   - src/worker/package.json       (worker, cosmetic)
#   - CHANGELOG.md                  (adds an Unreleased -> $NEW section header)
#
# Pass a bare semver (no leading "v"). The script reminds you to tag with "v".
#
# Example:
#   ./scripts/bump-version.sh 0.2.0
#
# Verify what changed before committing:
#   git diff --stat
#

set -euo pipefail

usage() {
    cat >&2 <<EOF
usage: $0 <semver>

  <semver>   Plain semver string, e.g. 0.2.0 or 0.2.0-beta.1 (NO leading v)

After running, review the diff, edit CHANGELOG.md if needed, then:
  git commit -am "chore: bump version to <semver>"
  git tag v<semver>
  git push origin main --tags
EOF
    exit 1
}

if [[ $# -ne 1 ]]; then
    usage
fi

NEW="$1"
if [[ ! "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.]+)?(\+[A-Za-z0-9.]+)?$ ]]; then
    echo "error: '$NEW' is not a valid semver (expected e.g. 0.2.0 or 0.2.0-beta.1)" >&2
    usage
fi

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

# Surgical awk edit of the first matching version line. Avoids the whole-file
# reformatting that `jq` does, which would churn unrelated JSON formatting.
tmp=$(mktemp)
replace_version_line() {
    local file="$1" pattern="$2" replacement="$3"
    awk -v pat="$pattern" -v repl="$replacement" '
        !done && match($0, pat) {
            $0 = repl
            done = 1
        }
        { print }
    ' "$file" > "$tmp"
    mv "$tmp" "$file"
}

# tauri.conf.json: canonical version per Tauri 2 schema
replace_version_line \
    src/tauri/tauri.conf.json \
    '^[[:space:]]*"version":[[:space:]]*"[^"]+"' \
    "  \"version\": \"$NEW\","

# Cargo manifests: first `version = "..."` line is the [package] one
for manifest in src/tauri/Cargo.toml src/agent/Cargo.toml; do
    replace_version_line \
        "$manifest" \
        '^version = "[^"]+"$' \
        "version = \"$NEW\""
done

# package.json files: top-level "version" key
for pkg in src/web/package.json src/worker/package.json; do
    replace_version_line \
        "$pkg" \
        '^[[:space:]]*"version":[[:space:]]*"[^"]+"' \
        "  \"version\": \"$NEW\","
done

# CHANGELOG.md: insert a new placeholder section above the latest entry.
# Idempotent for the same NEW (won't double-insert).
if [[ -f CHANGELOG.md ]] && ! grep -qF "## [$NEW] " CHANGELOG.md; then
    today=$(date -u +%Y-%m-%d)
    awk -v ver="$NEW" -v date="$today" '
        BEGIN { inserted = 0 }
        !inserted && /^## \[/ {
            print "## [" ver "] - " date
            print ""
            print "### Added"
            print "- "
            print ""
            print "### Changed"
            print "- "
            print ""
            inserted = 1
        }
        { print }
    ' CHANGELOG.md > "$tmp"
    mv "$tmp" CHANGELOG.md
fi

echo "Bumped to $NEW"
echo "Changed files:"
git diff --stat -- \
    src/tauri/tauri.conf.json \
    src/tauri/Cargo.toml \
    src/agent/Cargo.toml \
    src/web/package.json \
    src/worker/package.json \
    CHANGELOG.md
echo ""
echo "Next:"
echo "  1. Edit CHANGELOG.md to describe what changed."
echo "  2. git commit -am \"chore: bump version to $NEW\""
echo "  3. git tag v$NEW && git push origin main --tags"
