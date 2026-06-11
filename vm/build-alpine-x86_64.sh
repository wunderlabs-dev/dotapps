#!/bin/bash
# Builds Alpine Linux VM image for Intel Macs

set -e

VERSION="3.19"
ARCH="x86_64"
ISO_URL="https://dl-cdn.alpinelinux.org/alpine/v${VERSION}/releases/${ARCH}/alpine-virt-${VERSION}.0-${ARCH}.iso"
OUTPUT_DIR="$(dirname "$0")/output"
IMAGE_NAME="alpine-opnble-x86_64.qcow2"

mkdir -p "$OUTPUT_DIR"

echo "Downloading Alpine ISO..."
curl -L -o "$OUTPUT_DIR/alpine.iso" "$ISO_URL"

echo "Creating disk image..."
qemu-img create -f qcow2 "$OUTPUT_DIR/$IMAGE_NAME" 2G

echo ""
echo "To build automatically with Packer:"
echo "packer build alpine-x86_64.pkr.hcl"
