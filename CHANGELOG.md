# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0]

dotapps: a "Dropbox for vibe-coded apps". A developer packs a Dockerfile
project into a shareable artifact and publishes it to a registry; a
non-technical operator installs, updates, and runs it from a desktop launcher.

### Added

- `dotapps` CLI: `pack` builds an app's Docker image for `linux/arm64` and
  writes a `.apps` artifact (zstd tar of `manifest.json` + `image.tar`);
  `publish` uploads it to the registry
- `dotapps.json` project manifest (name, slug, version, icon, internal port,
  description)
- Cloudflare Worker + R2 registry at `registry.dotapps.club` with a small HTTP
  API for listing, resolving, and publishing app versions, plus presigned-URL
  and Worker-served blob paths
- Launcher with a Store tab (browse published apps) and a Library tab (installed
  apps)
- Install / open / stop / update of apps as podman containers inside a bundled
  Alpine VM, each in its own window
- Persistent per-app `/data` volume so app data survives updates
- `dotapps://{slug}` and `dotapps://{slug}@{version}` deep links that install,
  run, and open an app on click
- macOS support on Apple Silicon via vfkit and Virtualization.framework
- Example apps under `examples/` (`cafe-tracker`, `shift-board`) for the
  install / update / data-survival demo

### Notes

- dotapps is forked from the opnble launcher. Several internal identifiers keep
  the original name on purpose: the Rust crates (`opnble`, `opnble_lib`,
  `opnble-agent`), the bundled binary `opnble`, the in-VM container prefix
  `opnble-`, and the VirtioFS mount tag `opnble-repos`. They are baked into the
  VM image and build, and are not user-facing.
