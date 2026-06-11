#!/bin/sh
set -e

# Update and install packages
apk update
apk add --no-cache \
    podman \
    openssh-server \
    bash \
    git \
    nodejs \
    npm \
    shadow

# Enable cgroups v2
rc-update add cgroups default

# Configure SSH
ssh-keygen -A
echo "PermitRootLogin prohibit-password" >> /etc/ssh/sshd_config
echo "PasswordAuthentication no" >> /etc/ssh/sshd_config
rc-update add sshd default

# Create opnble user
useradd -m -s /bin/bash opnble
mkdir -p /home/opnble/.ssh
chmod 700 /home/opnble/.ssh
touch /home/opnble/.ssh/authorized_keys
chmod 600 /home/opnble/.ssh/authorized_keys
chown opnble:opnble /home/opnble/.ssh/authorized_keys

# Configure Podman for rootless
echo "opnble:100000:65536" >> /etc/subuid
echo "opnble:100000:65536" >> /etc/subgid

# Create mount point for repos
mkdir -p /repos
chown opnble:opnble /repos

# Auto-start Podman socket
mkdir -p /home/opnble/.config/systemd/user
mkdir -p /home/opnble/.config/containers
cat > /home/opnble/.config/containers/containers.conf << 'EOF'
[engine]
cgroup_manager = "cgroupfs"
events_logger = "file"
EOF

chown -R opnble:opnble /home/opnble/.config

echo "Alpine VM setup complete"
