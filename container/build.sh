#!/bin/bash
set -e

IMAGE_NAME="opnble-node"
IMAGE_TAG="20-alpine"

echo "Building container image..."
podman build -t ${IMAGE_NAME}:${IMAGE_TAG} .

echo "Saving image to tar..."
podman save ${IMAGE_NAME}:${IMAGE_TAG} -o ../assets/node-runner.tar

echo "Done! Image saved to assets/node-runner.tar"
