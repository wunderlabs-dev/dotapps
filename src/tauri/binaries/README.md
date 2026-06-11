# Sidecar binaries

This directory is populated at build time by `scripts/prepare-resources.sh`.
It holds the helper binaries Tauri ships as `externalBin` sidecars:

- `vfkit-<target-triple>` (macOS only)
- `gvproxy-<target-triple>` (macOS only)
- `cloudflared-<target-triple>` (all platforms)

The binaries themselves are gitignored. The `.gitkeep` keeps the directory
present in the tree so `tauri.conf.json` can reference relative paths even
before the first `prepare-resources.sh` run.

See `docs/beta-implementation-plan.md` Phase 1 for the full layout.
