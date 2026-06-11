#!/usr/bin/env bash
#
# upload-vm-image.sh <image-version-slug>
#
# Compress the locally built ~/.opnble/vm/alpine-base.img with zstd,
# compute SHA256 for both the compressed and decompressed forms, upload
# the compressed image and a manifest to the opnble-vm R2 bucket. The
# manifest is what the host crate (Phase 2 Rust client) reads at first
# launch to decide whether to download.
#
# Prerequisites:
#   - wrangler authenticated (`wrangler login` or CLOUDFLARE_API_TOKEN env).
#   - The opnble-vm R2 bucket created: `wrangler r2 bucket create opnble-vm`.
#   - zstd installed (Homebrew: `brew install zstd`; Linux pkg: `apt install zstd`).
#   - Local image present at ~/.opnble/vm/alpine-base.img (`make vm-image`).
#
# Example:
#   ./scripts/upload-vm-image.sh 2026-06-02-alpine-3.20
#

set -euo pipefail

# Resolve a wrangler invocation: prefer a globally installed wrangler so
# maintainers without the worker workspace can still upload, fall back to
# `npx --yes` so a fresh checkout works without `npm i -g`.
if command -v wrangler >/dev/null 2>&1; then
    WRANGLER=(wrangler)
else
    WRANGLER=(npx --yes wrangler)
fi

IMAGE_VERSION="${1:?usage: $0 <image-version-slug e.g. 2026-06-02-alpine-3.20>}"

RAW="$HOME/.opnble/vm/alpine-base.img"
if [[ ! -f "$RAW" ]]; then
    echo "error: missing $RAW. Run 'make vm-image' first." >&2
    exit 1
fi

OUT_DIR=$(mktemp -d)
trap 'rm -rf "$OUT_DIR"' EXIT

KEY="alpine-base-${IMAGE_VERSION}.img.zst"
COMPRESSED="$OUT_DIR/$KEY"
MANIFEST="$OUT_DIR/manifest.json"

echo "Compressing $RAW (this can take a few minutes)..."
zstd --long --threads=0 -19 --force -o "$COMPRESSED" "$RAW"

sha_of() {
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        sha256sum "$1" | awk '{print $1}'
    fi
}

size_of() {
    if command -v gstat >/dev/null 2>&1; then
        gstat -c %s "$1"
    elif command -v stat >/dev/null 2>&1 && stat -c %s "$1" >/dev/null 2>&1; then
        stat -c %s "$1"
    else
        stat -f %z "$1"
    fi
}

UNCOMPRESSED_SHA=$(sha_of "$RAW")
COMPRESSED_SHA=$(sha_of "$COMPRESSED")
UNCOMPRESSED_SIZE=$(size_of "$RAW")
COMPRESSED_SIZE=$(size_of "$COMPRESSED")

echo "Uncompressed: $UNCOMPRESSED_SIZE bytes, sha256 $UNCOMPRESSED_SHA"
echo "Compressed:   $COMPRESSED_SIZE bytes, sha256 $COMPRESSED_SHA"

echo "Uploading $KEY to R2 bucket opnble-vm..."
# `--remote` targets the real Cloudflare R2 bucket. Without it, wrangler
# writes to its local sqlite simulator at .wrangler/state/ and the worker
# never sees the object.
"${WRANGLER[@]}" r2 object put "opnble-vm/$KEY" --file "$COMPRESSED" --remote

cat > "$MANIFEST" <<EOF
{
  "schemaVersion": 1,
  "imageVersion": "${IMAGE_VERSION}",
  "url": "https://openable.dev/vm/${KEY}",
  "compression": "zstd",
  "sha256": "${COMPRESSED_SHA}",
  "uncompressedSha256": "${UNCOMPRESSED_SHA}",
  "compressedSize": ${COMPRESSED_SIZE},
  "uncompressedSize": ${UNCOMPRESSED_SIZE},
  "notes": ""
}
EOF

echo "Uploading manifest.json to R2..."
"${WRANGLER[@]}" r2 object put "opnble-vm/manifest.json" --file "$MANIFEST" --remote

echo ""
echo "Done. Verify with:"
echo "  curl -s https://openable.dev/vm/manifest.json | jq ."
echo "  curl -sI https://openable.dev/vm/${KEY} | head"
