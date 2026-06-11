#!/bin/bash
# Build Alpine Linux tar for WSL import
# Uses Docker to create and configure an Alpine container, then exports it

set -e

OUTPUT_DIR="$(dirname "$0")/output"
CONTAINER_NAME="alpine-wsl-builder"
TAR_NAME="alpine-wsl.tar"

mkdir -p "$OUTPUT_DIR"

echo "Creating Alpine container..."
docker run --name "$CONTAINER_NAME" -d alpine:3.19 sleep infinity

echo "Running setup script..."
docker cp "$(dirname "$0")/wsl-setup.sh" "$CONTAINER_NAME:/setup.sh"
docker exec "$CONTAINER_NAME" sh /setup.sh

echo "Exporting container..."
docker export "$CONTAINER_NAME" > "$OUTPUT_DIR/$TAR_NAME"

echo "Cleaning up..."
docker rm -f "$CONTAINER_NAME"

echo "Done! WSL image saved to $OUTPUT_DIR/$TAR_NAME"
ls -lh "$OUTPUT_DIR/$TAR_NAME"
