#!/usr/bin/env bash
set -euo pipefail

# Args:
#   $1 — version tag (e.g., v0.1.1)
#   $2 — bundle directory (contains the .app.tar.gz and .sig)
#   $3 — output path for latest.json fragment (or full manifest)
#   $4 — platform key for Tauri updater (default: darwin-aarch64)
#
# The output is a Tauri updater manifest with a single platform entry.
# Multi-platform releases (aarch64 + x86_64 mac, windows, linux) call this
# once per arch and merge the fragments with `jq -s '.[0] * .[1] * ...'`
# in the release workflow.

if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
    echo "usage: $0 <tag> <bundle-dir> <output-json> [platform-key]" >&2
    echo "  platform-key examples: darwin-aarch64, darwin-x86_64, windows-x86_64, linux-x86_64" >&2
    exit 1
fi

TAG="$1"
BUNDLE_DIR="$2"
OUTPUT="$3"
PLATFORM="${4:-darwin-aarch64}"

# Base URL for download endpoints. Releases are served from the Cloudflare
# Worker (`opnble-tunnel-api`) backed by the `opnble-releases` R2 bucket so
# the private GitHub repo is not exposed to update clients. Override per
# environment if a fork ships from a different host.
RELEASE_BASE_URL="${RELEASE_BASE_URL:-https://openable.dev/releases}"

# Strip leading 'v' from tag for SemVer
VERSION="${TAG#v}"

SIG_FILE=$(find "$BUNDLE_DIR" -name "*.app.tar.gz.sig" | head -n1)
ARCHIVE=$(find "$BUNDLE_DIR" -name "*.app.tar.gz" -not -name "*.sig" | head -n1)

if [ -z "$SIG_FILE" ] || [ -z "$ARCHIVE" ]; then
    echo "missing .app.tar.gz or .sig in $BUNDLE_DIR" >&2
    exit 1
fi

ARCHIVE_NAME=$(basename "$ARCHIVE")
SIGNATURE=$(cat "$SIG_FILE")
PUB_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
DOWNLOAD_URL="${RELEASE_BASE_URL}/${TAG}/${ARCHIVE_NAME}"

# Embed signature as a JSON-safe string. The signature is multi-line
# minisign output; jq -Rs ingests as a single string.
jq -n \
  --arg version "$VERSION" \
  --arg pub_date "$PUB_DATE" \
  --arg signature "$SIGNATURE" \
  --arg url "$DOWNLOAD_URL" \
  --arg platform "$PLATFORM" \
  '{
    version: $version,
    notes: "",
    pub_date: $pub_date,
    platforms: ({} | .[$platform] = {
      signature: $signature,
      url: $url
    })
  }' > "$OUTPUT"

echo "wrote $OUTPUT for $TAG ($PLATFORM)"
