//! Vibox app management: registry-published podman apps running in the VM.
//!
//! Apps are published to the vibox registry as `.vibox` artifacts (a
//! zstd-compressed tar of `manifest.json` + `image.tar`), installed under
//! `~/.vibox/repos/{slug}/`, and run as podman containers inside the VM via
//! the agent's `ExecHost` RPC. Installed state lives at `~/.vibox/apps.json`.

pub mod registry;
pub mod store;
pub mod types;
