#!/bin/bash
# Builds Alpine Linux VM image for Apple Silicon Macs
# Requires: qemu-img, and Alpine ISO

set -e

VERSION="3.19"
ARCH="aarch64"
ISO_URL="https://dl-cdn.alpinelinux.org/alpine/v${VERSION}/releases/${ARCH}/alpine-virt-${VERSION}.0-${ARCH}.iso"
OUTPUT_DIR="$(dirname "$0")/output"
IMAGE_NAME="alpine-opnble-arm64.img"

mkdir -p "$OUTPUT_DIR"

echo "Downloading Alpine ISO..."
curl -L -o "$OUTPUT_DIR/alpine.iso" "$ISO_URL"

echo "Creating disk image..."
qemu-img create -f raw "$OUTPUT_DIR/$IMAGE_NAME" 2G

echo ""
echo "=== Manual steps required ==="
echo "1. Boot the ISO in UTM or similar"
echo "2. Run: setup-alpine"
echo "3. Copy setup-alpine.sh to VM and run it"
echo "4. Shutdown and extract the disk image"
echo ""
echo "For automated builds, use Packer with QEMU"
