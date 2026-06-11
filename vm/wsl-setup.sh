#!/bin/sh
# Run inside Alpine to prepare for WSL export
# This script configures Alpine Linux for use as a WSL2 distro with Podman

set -e

# Update and install packages
apk update
apk add --no-cache \
    podman \
    fuse-overlayfs \
    slirp4netns \
    bash \
    nodejs \
    npm \
    git

# Configure Podman for cgroupfs (no systemd in WSL)
mkdir -p /etc/containers
cat > /etc/containers/containers.conf << 'EOF'
[engine]
cgroup_manager = "cgroupfs"
events_logger = "file"
EOF

cat > /etc/containers/storage.conf << 'EOF'
[storage]
driver = "overlay"
runroot = "/run/containers/storage"
graphroot = "/var/lib/containers/storage"

[storage.options.overlay]
mount_program = "/usr/bin/fuse-overlayfs"
EOF

# Create opnble user with subuid/subgid mapping
adduser -D -s /bin/bash opnble
echo "opnble:100000:65536" >> /etc/subuid
echo "opnble:100000:65536" >> /etc/subgid

# Configure rootless Podman for opnble user
mkdir -p /home/opnble/.config/containers
cp /etc/containers/containers.conf /home/opnble/.config/containers/
cp /etc/containers/storage.conf /home/opnble/.config/containers/
chown -R opnble:opnble /home/opnble/.config

# Create mount point for repos
mkdir -p /repos
chown opnble:opnble /repos

echo "WSL Alpine setup complete"
