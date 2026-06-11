#!/usr/bin/env bash
# Seeds ~/.vibox/vm with the locally-built opnble base image so first launch
# skips the 4GB download. APFS clonefile makes the copy instant.
set -euo pipefail

SRC="$HOME/.opnble/vm/alpine-base.img"
DST_DIR="$HOME/.vibox/vm"
[ -f "$SRC" ] || { echo "missing $SRC — build or download it via opnble first"; exit 1; }

mkdir -p "$DST_DIR"
[ -f "$DST_DIR/alpine-base.img" ] || cp -c "$SRC" "$DST_DIR/alpine-base.img"

# Stamp the version the live manifest advertises so the downloader's version
# check (vm/image_downloader.rs status()) is satisfied. The stamp struct
# (InstalledStamp) serializes snake_case; the remote manifest is camelCase.
VERSION=$(curl -fsS --max-time 10 https://openable.dev/vm/manifest.json \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["imageVersion"])' || echo "local-seed")
printf '{"image_version":"%s"}\n' "$VERSION" > "$DST_DIR/image-version.json"
echo "seeded $DST_DIR (version: $VERSION)"
