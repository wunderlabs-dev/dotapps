# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-10

### Added

- Clone and run React/Next.js projects from GitHub and GitLab URLs
- OAuth authentication for private repositories (GitHub and GitLab)
- Multi-project support with sidebar project list
- Project context menu with branch switching, pull latest, restart, and remove options
- Live log streaming from running containers
- Toast notification system for user feedback
- Setup screen for first-time users
- macOS support:
  - ARM Macs using Apple Virtualization.framework (fast boot)
  - Intel Macs using QEMU fallback
- Linux support with native Podman containers
- Windows support:
  - WSL2 integration (preferred)
  - QEMU fallback when WSL2 is unavailable
- Automatic port management for running projects
- Branch switching and pull latest functionality
- Embedded webview for viewing running applications
- Persistent project storage

### Known Issues

- QEMU fallback on Windows and Intel Macs is slower than native virtualization
- Large repositories may take additional time to sync on macOS
- First-time setup requires downloading VM images which may take a few minutes
