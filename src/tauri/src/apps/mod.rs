//! dotapps app management: registry-published podman apps running in the VM.
//!
//! Apps are published to the dotapps registry as `.apps` artifacts (a
//! zstd-compressed tar of `manifest.json` + `image.tar`), installed under
//! `~/.dotapps/repos/{slug}/`, and run as podman containers inside the VM via
//! the agent's `ExecHost` RPC. Installed state lives at `~/.dotapps/apps.json`.

pub mod registry;
pub mod store;
pub mod types;

#[cfg(target_os = "macos")]
pub mod commands;

use std::collections::HashMap;

use tokio::task::JoinHandle;

/// Port-forward handles for running dotapps apps, keyed by slug.
///
/// Managed as Tauri state so the run/stop commands and the startup auto-run
/// task share the same forwarders (mirrors `VmRuntime::port_forwards`).
#[derive(Default)]
pub struct AppForwards(pub tokio::sync::Mutex<HashMap<String, JoinHandle<()>>>);
