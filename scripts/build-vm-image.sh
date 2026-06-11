#!/bin/bash
# scripts/build-vm-image.sh
#
# Builds a minimal Alpine Linux VM image for Opnble with:
# - opnble-agent (ttrpc server on vsock port 1024)
# - Podman (container runtime)
# - VirtioFS mount at /repos
# - No SSH (all communication via vsock)
#
# Prerequisites: qemu-system-aarch64, qemu-img, zig, cargo-zigbuild
# Usage: make vm-image

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_DIR/build/vm"
OUTPUT_DIR="$HOME/.opnble/vm"
ALPINE_VERSION="3.20"
ALPINE_IMAGE="nocloud_alpine-3.20.1-aarch64-uefi-cloudinit-r0.qcow2"
ALPINE_URL="https://dl-cdn.alpinelinux.org/alpine/v${ALPINE_VERSION}/releases/cloud/${ALPINE_IMAGE}"
AGENT_TARGET="aarch64-unknown-linux-musl"
QEMU_RAM="2048"
QEMU_CPUS="2"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}==>${NC} $*"; }
warn()  { echo -e "${YELLOW}==> WARNING:${NC} $*"; }
error() { echo -e "${RED}==> ERROR:${NC} $*" >&2; exit 1; }

# --- Preflight checks ---

check_deps() {
    local missing=()
    command -v qemu-system-aarch64 >/dev/null || missing+=(qemu-system-aarch64)
    command -v qemu-img >/dev/null            || missing+=(qemu-img)
    command -v docker >/dev/null              || missing+=(docker)

    if [ ${#missing[@]} -ne 0 ]; then
        error "Missing dependencies: ${missing[*]}. Install via: brew install qemu docker"
    fi

    # Verify Docker is running
    if ! docker info >/dev/null 2>&1; then
        error "Docker is not running. Start Docker Desktop first."
    fi
}

# --- Step 1: Cross-compile the agent ---

build_agent() {
    info "Cross-compiling opnble-agent for $AGENT_TARGET via Docker..."

    local agent_bin="$BUILD_DIR/opnble-agent"
    mkdir -p "$BUILD_DIR"

    # Build inside Debian container with Rust toolchain.
    # Source is mounted read-only; cargo writes to a separate volume for /build/target.
    docker run --rm \
        --platform linux/arm64 \
        -v "$PROJECT_DIR:/project:ro" \
        -v "$BUILD_DIR:/output" \
        -e CARGO_TARGET_DIR=/build/target \
        -w /project/src/agent \
        rust:slim-bookworm \
        sh -c "
            rustup target add $AGENT_TARGET &&
            apt-get update -qq && apt-get install -y -qq musl-tools protobuf-compiler >/dev/null 2>&1 &&
            cargo build --release --target $AGENT_TARGET &&
            cp /build/target/$AGENT_TARGET/release/opnble-agent /output/opnble-agent
        "

    if [ ! -f "$agent_bin" ]; then
        error "Agent binary not found at $agent_bin"
    fi
    info "Agent binary: $agent_bin ($(du -h "$agent_bin" | cut -f1))"
}

# --- Step 2: Download Alpine cloud image ---

download_alpine() {
    mkdir -p "$BUILD_DIR"

    if [ -f "$BUILD_DIR/$ALPINE_IMAGE" ]; then
        info "Alpine cloud image already cached"
        return
    fi

    info "Downloading Alpine $ALPINE_VERSION cloud image..."
    curl -L -o "$BUILD_DIR/$ALPINE_IMAGE" "$ALPINE_URL"
    info "Download complete"
}

# --- Step 3: Create cloud-init seed ISO via Docker ---

create_seed_iso() {
    info "Creating cloud-init seed ISO..."

    local seed_dir="$BUILD_DIR/seed"
    rm -rf "$seed_dir"
    mkdir -p "$seed_dir"

    cat > "$seed_dir/meta-data" << 'META'
instance-id: opnble-vm
local-hostname: opnble
META

    # The user-data mounts the 9p share, copies the agent, configures services,
    # then powers off. NoCloud datasource avoids the 4-min metadata timeout.
    cat > "$seed_dir/user-data" << 'USERDATA'
#cloud-config

# Root password for debugging (SSH is removed, only serial console access)
chpasswd:
  expire: false
  users:
    - name: root
      password: opnble
      type: text

packages:
  - podman
  - crun
  - fuse-overlayfs

write_files:
  - path: /etc/containers/registries.conf
    content: |
      unqualified-search-registries = ["docker.io"]

  - path: /etc/init.d/opnble-agent
    permissions: "0755"
    content: |
      #!/sbin/openrc-run
      name="opnble-agent"
      description="Opnble VM Agent (ttrpc over vsock)"
      command="/usr/local/bin/opnble-agent"
      command_background="yes"
      pidfile="/run/opnble-agent.pid"
      output_log="/var/log/opnble-agent.log"
      error_log="/var/log/opnble-agent.log"

      start_pre() {
          # Load vsock kernel modules before starting agent
          modprobe vsock 2>/dev/null || true
          modprobe vmw_vsock_virtio_transport 2>/dev/null || true
          modprobe vhost_vsock 2>/dev/null || true
          # Podman is started by the agent itself (ensure_podman_socket)
          # with proper liveness checks and retry logic. Do NOT start it
          # here: backgrounding it creates zombies and stale sockets.
      }

      depend() {
          need net cgroups
          after firewall
      }

  - path: /etc/modules-load.d/vsock.conf
    content: |
      vsock
      vmw_vsock_virtio_transport

runcmd:
  # Load vsock modules now (and on every boot via modules-load.d)
  - modprobe vsock 2>/dev/null || true
  - modprobe vmw_vsock_virtio_transport 2>/dev/null || true

  # Mount the build share and install agent
  - mkdir -p /mnt/build
  - mount -t 9p -o trans=virtio,version=9p2000.L build /mnt/build
  - cp /mnt/build/opnble-agent /usr/local/bin/opnble-agent
  - chmod 755 /usr/local/bin/opnble-agent
  - umount /mnt/build

  # Create repos mount point and fstab entry
  - mkdir -p /repos
  - echo "opnble-repos /repos virtiofs defaults,nofail 0 0" >> /etc/fstab

  # Configure networking for runtime (vfkit+gvproxy).
  # Written here (not write_files) so it doesn't interfere with QEMU
  # build networking. The post-up ARP entry pins gvproxy's gateway MAC
  # because ARP resolution over the vfkit unixgram transport is unreliable.
  - printf 'auto lo\niface lo inet loopback\n\nauto eth0\niface eth0 inet dhcp\n  post-up arp -s 192.168.127.1 5a:94:ef:e4:0c:dd\n' > /etc/network/interfaces
  - rc-update add networking default

  # Enable cgroups v2 (required by podman)
  - sed -i 's/^#rc_cgroup_mode=.*/rc_cgroup_mode="unified"/' /etc/rc.conf || true
  - rc-update add cgroups default

  # Pre-pull Node.js image so first project start is instant
  - podman pull docker.io/library/node:20-alpine

  # Enable agent on boot
  - rc-update add opnble-agent default

  # Remove SSH (all comms via vsock)
  - apk del openssh-server openssh-client 2>/dev/null || true
  - rc-update del sshd default 2>/dev/null || true

  # Disable cloud-init (setup is done, no need on future boots)
  - rc-update del cloud-init-local default 2>/dev/null || true
  - rc-update del cloud-init default 2>/dev/null || true
  - rc-update del cloud-config default 2>/dev/null || true
  - rc-update del cloud-final default 2>/dev/null || true

  # Clean up
  - rm -rf /var/cache/apk/*

  # Signal done
  - echo "=== OPNBLE SETUP COMPLETE ==="
  - poweroff

final_message: "Opnble VM image build complete in $UPTIME seconds"
USERDATA

    # Build ISO using genisoimage inside Docker (hdiutil doesn't produce
    # an ISO that Alpine's cloud-init NoCloud datasource can read)
    local iso_path="$BUILD_DIR/seed.iso"
    rm -f "$iso_path"
    docker run --rm \
        -v "$seed_dir:/seed:ro" \
        -v "$BUILD_DIR:/output" \
        debian:bookworm-slim \
        sh -c "apt-get update -qq && apt-get install -y -qq genisoimage >/dev/null 2>&1 && \
               genisoimage -output /output/seed.iso -volid cidata -joliet -rock /seed/"

    info "Seed ISO: $iso_path"
}

# --- Step 4: Boot QEMU and run cloud-init ---

customize_image() {
    info "Preparing working copy of Alpine image..."

    local work_image="$BUILD_DIR/alpine-work.qcow2"
    cp "$BUILD_DIR/$ALPINE_IMAGE" "$work_image"

    # Resize image to 4GB (default cloud image is ~256MB, too small for podman)
    qemu-img resize -f qcow2 "$work_image" 4G

    # Copy agent binary into the shared directory (created by create_seed_iso)
    local share_dir="$BUILD_DIR/share"
    mkdir -p "$share_dir"
    cp "$BUILD_DIR/opnble-agent" "$share_dir/"

    info "Booting Alpine in QEMU for customization (this takes ~2 min)..."

    # Determine QEMU firmware path
    local qemu_efi=""
    for path in \
        /opt/homebrew/share/qemu/edk2-aarch64-code.fd \
        /usr/share/qemu/edk2-aarch64-code.fd \
        /usr/local/share/qemu/edk2-aarch64-code.fd; do
        if [ -f "$path" ]; then
            qemu_efi="$path"
            break
        fi
    done
    if [ -z "$qemu_efi" ]; then
        error "QEMU EFI firmware not found. Install qemu with: brew install qemu"
    fi

    # Boot headless with NoCloud seed ISO. Cloud-init reads the seed,
    # runs our runcmd (install agent, configure services), then powers off.
    # -no-reboot makes QEMU exit when guest calls poweroff.
    qemu-system-aarch64 \
        -machine virt,accel=hvf \
        -cpu host \
        -m "$QEMU_RAM" \
        -smp "$QEMU_CPUS" \
        -bios "$qemu_efi" \
        -drive "file=$work_image,if=virtio,format=qcow2" \
        -drive "file=$BUILD_DIR/seed.iso,if=virtio,media=cdrom" \
        -virtfs "local,path=$share_dir,mount_tag=build,security_model=mapped-xattr,id=build" \
        -netdev user,id=net0 \
        -device virtio-net-pci,netdev=net0 \
        -nographic \
        -no-reboot \
        2>&1 | tee "$BUILD_DIR/qemu-build.log" || true

    if grep -q "OPNBLE SETUP COMPLETE" "$BUILD_DIR/qemu-build.log" 2>/dev/null; then
        info "Setup completed successfully"
    else
        error "Setup did not complete. Check $BUILD_DIR/qemu-build.log"
    fi
}

# --- Step 5: Convert and install ---

install_image() {
    info "Converting image to raw format for vfkit..."

    mkdir -p "$OUTPUT_DIR"

    local work_image="$BUILD_DIR/alpine-work.qcow2"
    local base_image="$OUTPUT_DIR/alpine-base.img"

    # Back up existing base image if present
    if [ -f "$base_image" ]; then
        mv "$base_image" "${base_image}.bak"
        info "Backed up previous base image"
    fi

    qemu-img convert -f qcow2 -O raw "$work_image" "$base_image"

    # Mark base image read-only to prevent accidental writes
    chmod 444 "$base_image"

    # Remove stale runtime clone and EFI store
    rm -f "$OUTPUT_DIR/alpine.img"
    rm -f "$OUTPUT_DIR/vfkit-efi-store"

    # Copy seed for reference
    cp "$BUILD_DIR/seed.iso" "$OUTPUT_DIR/seed.iso"

    local size
    size=$(du -h "$base_image" | cut -f1)
    info "Base VM image installed: $base_image ($size)"
    info "Runtime clone will be created on first 'make dev'"
}

# --- Main ---

main() {
    info "Building Opnble VM image"
    echo ""

    check_deps
    build_agent
    download_alpine
    create_seed_iso
    customize_image
    install_image

    echo ""
    info "Done! Run 'make dev' to start with the new VM image."
}

main "$@"
