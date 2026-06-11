#!/bin/bash
#
# prepare-resources.sh
#
# Prepares platform-specific resources for Tauri bundling.
# Detects the current platform and architecture, then copies
# appropriate VM/container images to the resources directory.
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RESOURCES_DIR="$PROJECT_ROOT/src/tauri/resources"
# Tauri externalBin layout: <crate>/binaries/<name>-<target-triple>. Tauri
# installs these into <bundle>/Contents/MacOS/<name> at build time and
# codesigns them with the host app's identity.
BINARIES_DIR="$PROJECT_ROOT/src/tauri/binaries"
mkdir -p "$BINARIES_DIR"

# Pull the app version from the canonical Tauri config so the manifest stays
# in lockstep with the binary. See scripts/bump-version.sh.
APP_VERSION=$(jq -r .version "$PROJECT_ROOT/src/tauri/tauri.conf.json")

# Ensure resources directory exists
mkdir -p "$RESOURCES_DIR"

# Detect platform
detect_platform() {
    case "$(uname -s)" in
        Linux*)     echo "linux";;
        Darwin*)    echo "macos";;
        CYGWIN*|MINGW*|MSYS*) echo "windows";;
        *)          echo "unknown";;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)   echo "x86_64";;
        aarch64|arm64)  echo "aarch64";;
        *)              echo "unknown";;
    esac
}

PLATFORM=$(detect_platform)
ARCH=$(detect_arch)

echo "=== Opnble Resource Preparation ==="
echo "Platform: $PLATFORM"
echo "Architecture: $ARCH"
echo "Resources directory: $RESOURCES_DIR"
echo ""

# Pinned helper-binary versions. Bump intentionally via a hygiene PR after
# testing the new release on a maintainer machine. See docs/release-setup.md
# (Phase 1 of the beta plan) for the rotation procedure.
VFKIT_VERSION="v0.6.3"
# vfkit ships a universal Mach-O signed ad-hoc by upstream with the
# com.apple.security.virtualization entitlement. We re-sign with the Opnble
# Developer ID when Tauri bundles it as an externalBin sidecar.
VFKIT_UNIVERSAL_SHA256="19d0695d40d996ec38529a22b73cdaa84ff67ba15f4b44927292e7fe885cee0e"

GVPROXY_VERSION="v0.8.9"
# gvproxy-darwin is a single universal binary (lipo'd amd64+arm64), unsigned
# upstream. Same re-signing path as vfkit.
GVPROXY_DARWIN_UNIVERSAL_SHA256="c6f7b4bc7f21bf810b5cf54e04d979b014c5d96472a03a9e97fe62a00940067c"

CLOUDFLARED_VERSION="2026.5.2"
CLOUDFLARED_DARWIN_ARM64_TGZ_SHA256="ba94054c9fd4297645093d59d51442e5e546d07bb0516120e694a13d5b216d38"
CLOUDFLARED_DARWIN_AMD64_TGZ_SHA256="7240f709506bc2c1eb9da4d89cf2555499c60280ecb854b7d80e8f17d4b7903d"
CLOUDFLARED_LINUX_AMD64_SHA256="5286698547f03df745adb2355f04c12dde52ef425491e81f433642d695521886"
CLOUDFLARED_LINUX_ARM64_SHA256="5a4e8ce2701105271412059f44b6a0bf1ae4542b4d98ff3180c0c019443a5815"
CLOUDFLARED_WINDOWS_AMD64_SHA256="20b9638f685333d623798e733effbad2487093f15ba592f6c7752360ff3b7ab7"

# Verify a file's SHA256 against an expected hex digest. Removes the file
# and exits non-zero on mismatch so we fail loud instead of shipping a
# corrupted helper.
verify_sha256() {
    local file="$1" expected="$2"
    local actual
    actual=$(shasum -a 256 "$file" 2>/dev/null | awk '{print $1}')
    if [ -z "$actual" ]; then
        actual=$(sha256sum "$file" 2>/dev/null | awk '{print $1}')
    fi
    if [ "$actual" != "$expected" ]; then
        echo "  - ERROR: sha256 mismatch for $file" >&2
        echo "    expected: $expected" >&2
        echo "    actual:   $actual" >&2
        rm -f "$file"
        exit 1
    fi
}

# Download a single file with SHA verification. Idempotent: re-use an existing
# file when its hash already matches the pin.
fetch_pinned() {
    local url="$1" dest="$2" want="$3"
    if [ -f "$dest" ]; then
        local have
        have=$(shasum -a 256 "$dest" 2>/dev/null | awk '{print $1}')
        if [ -z "$have" ]; then
            have=$(sha256sum "$dest" 2>/dev/null | awk '{print $1}')
        fi
        if [ "$have" = "$want" ]; then
            echo "  - $(basename "$dest"): already current, skipping"
            return 0
        fi
        echo "  - $(basename "$dest"): sha mismatch, redownloading"
        rm -f "$dest"
    fi
    echo "  - downloading $url"
    curl -fsSL --retry 3 -o "$dest.tmp" "$url"
    verify_sha256 "$dest.tmp" "$want"
    mv "$dest.tmp" "$dest"
    chmod +x "$dest"
}

# Download vfkit and gvproxy universal binaries once, then drop them at the
# externalBin sidecar paths Tauri expects. Universal Mach-O works on both
# arm64 and x86_64 slices, so one file backs both target triples.
download_macos_vm_helpers() {
    local vfkit_universal="$BINARIES_DIR/.vfkit-universal-darwin"
    fetch_pinned \
        "https://github.com/crc-org/vfkit/releases/download/${VFKIT_VERSION}/vfkit" \
        "$vfkit_universal" \
        "$VFKIT_UNIVERSAL_SHA256"
    cp -f "$vfkit_universal" "$BINARIES_DIR/vfkit-aarch64-apple-darwin"
    cp -f "$vfkit_universal" "$BINARIES_DIR/vfkit-x86_64-apple-darwin"
    chmod +x "$BINARIES_DIR"/vfkit-*-apple-darwin

    local gvproxy_universal="$BINARIES_DIR/.gvproxy-darwin-universal"
    fetch_pinned \
        "https://github.com/containers/gvisor-tap-vsock/releases/download/${GVPROXY_VERSION}/gvproxy-darwin" \
        "$gvproxy_universal" \
        "$GVPROXY_DARWIN_UNIVERSAL_SHA256"
    cp -f "$gvproxy_universal" "$BINARIES_DIR/gvproxy-aarch64-apple-darwin"
    cp -f "$gvproxy_universal" "$BINARIES_DIR/gvproxy-x86_64-apple-darwin"
    chmod +x "$BINARIES_DIR"/gvproxy-*-apple-darwin
}

# Place cloudflared at the externalBin sidecar path for the current host.
# The Cloudflare release is a tarball on macOS (single inner "cloudflared")
# and a bare binary elsewhere.
download_cloudflared_sidecar() {
    case "$PLATFORM/$ARCH" in
        macos/aarch64)
            local tgz tmp dest
            tmp=$(mktemp /tmp/cloudflared-XXXXXX.tgz)
            fetch_pinned \
                "https://github.com/cloudflare/cloudflared/releases/download/${CLOUDFLARED_VERSION}/cloudflared-darwin-arm64.tgz" \
                "$tmp" \
                "$CLOUDFLARED_DARWIN_ARM64_TGZ_SHA256"
            dest="$BINARIES_DIR/cloudflared-aarch64-apple-darwin"
            tar xzf "$tmp" -O cloudflared > "$dest"
            chmod +x "$dest"
            rm -f "$tmp"
            ;;
        macos/x86_64)
            local tmp dest
            tmp=$(mktemp /tmp/cloudflared-XXXXXX.tgz)
            fetch_pinned \
                "https://github.com/cloudflare/cloudflared/releases/download/${CLOUDFLARED_VERSION}/cloudflared-darwin-amd64.tgz" \
                "$tmp" \
                "$CLOUDFLARED_DARWIN_AMD64_TGZ_SHA256"
            dest="$BINARIES_DIR/cloudflared-x86_64-apple-darwin"
            tar xzf "$tmp" -O cloudflared > "$dest"
            chmod +x "$dest"
            rm -f "$tmp"
            ;;
        linux/x86_64)
            fetch_pinned \
                "https://github.com/cloudflare/cloudflared/releases/download/${CLOUDFLARED_VERSION}/cloudflared-linux-amd64" \
                "$BINARIES_DIR/cloudflared-x86_64-unknown-linux-gnu" \
                "$CLOUDFLARED_LINUX_AMD64_SHA256"
            ;;
        linux/aarch64)
            fetch_pinned \
                "https://github.com/cloudflare/cloudflared/releases/download/${CLOUDFLARED_VERSION}/cloudflared-linux-arm64" \
                "$BINARIES_DIR/cloudflared-aarch64-unknown-linux-gnu" \
                "$CLOUDFLARED_LINUX_ARM64_SHA256"
            ;;
        windows/*)
            fetch_pinned \
                "https://github.com/cloudflare/cloudflared/releases/download/${CLOUDFLARED_VERSION}/cloudflared-windows-amd64.exe" \
                "$BINARIES_DIR/cloudflared-x86_64-pc-windows-msvc.exe" \
                "$CLOUDFLARED_WINDOWS_AMD64_SHA256"
            ;;
        *)
            echo "  - WARNING: no cloudflared download URL for $PLATFORM/$ARCH"
            return 1
            ;;
    esac
}

# Copy container image config (Dockerfile/Containerfile) into resources.
CONTAINER_DIR="$PROJECT_ROOT/container"
if [ -d "$CONTAINER_DIR" ]; then
    echo "Copying container configuration..."
    cp -r "$CONTAINER_DIR"/* "$RESOURCES_DIR/" 2>/dev/null || true
fi

# Copy VM cloud-init scripts into resources (non-binary, platform-shared).
VM_DIR="$PROJECT_ROOT/vm"
if [ -d "$VM_DIR" ]; then
    echo "Copying VM cloud-init scripts..."
    for f in "$VM_DIR/user-data" "$VM_DIR/meta-data"; do
        if [ -f "$f" ]; then
            cp "$f" "$RESOURCES_DIR/"
            echo "  - Copied $(basename "$f")"
        fi
    done
    for script in "$VM_DIR"/*.sh; do
        if [ -f "$script" ]; then
            cp "$script" "$RESOURCES_DIR/"
            echo "  - Copied $(basename "$script")"
        fi
    done
fi

# Host-platform helper binaries: download into src/tauri/binaries/ where
# Tauri's externalBin pipeline expects them. The bundler then installs them
# into <bundle>/Contents/MacOS/ and codesigns them with the host identity.
echo "Downloading helper binaries..."
case "$PLATFORM" in
    macos)
        download_macos_vm_helpers
        download_cloudflared_sidecar
        ;;
    linux)
        echo "  - Linux uses native Podman; no vfkit/gvproxy needed"
        download_cloudflared_sidecar
        ;;
    windows)
        echo "  - Windows uses WSL2; no vfkit/gvproxy needed"
        download_cloudflared_sidecar
        ;;
    *)
        echo "  - WARNING: unknown platform '$PLATFORM', skipping helper download"
        ;;
esac

# Create a manifest file for runtime resource detection
cat > "$RESOURCES_DIR/manifest.json" << EOF
{
  "platform": "$PLATFORM",
  "architecture": "$ARCH",
  "prepared_at": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "version": "$APP_VERSION"
}
EOF
echo ""
echo "Created resource manifest"

# List what landed where
echo ""
echo "=== Prepared Resources ==="
ls -la "$RESOURCES_DIR/" 2>/dev/null || true

echo ""
echo "=== Sidecar Binaries (externalBin) ==="
ls -la "$BINARIES_DIR/" 2>/dev/null || true

echo ""
echo "Resource preparation complete!"
