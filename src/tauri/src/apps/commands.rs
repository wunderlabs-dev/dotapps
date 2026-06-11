//! Vibox app lifecycle Tauri commands (frozen wire contract: six commands).
//!
//! Apps install from the registry into `~/.vibox/repos/{slug}/` (`VirtioFS`
//! mounts that at `/repos/{slug}/` in the VM) and run as podman containers
//! named `vibox-{slug}` with a persistent `vibox-{slug}-data` volume at
//! `/data`. All podman interaction goes through the agent's `ExecHost` RPC.

use std::sync::Arc;

use tauri::{Manager, State};

use super::registry;
use super::store::{not_installed, AppStore};
use super::types::{validate_slug, InstalledApp, StoreApp};
use super::AppForwards;
use crate::constants::paths;
use crate::error::AppError;
use crate::vm::VmLifecycle;

/// `GET /v1/apps` from the registry: everything installable.
#[tauri::command]
#[specta::specta]
pub async fn vibox_registry_apps() -> Result<Vec<StoreApp>, AppError> {
    registry::fetch_store_apps().await
}

/// Installed apps from the local store (store state is the source of truth
/// for `running`; no podman round-trip).
#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn vibox_installed_apps(
    store: State<Arc<AppStore>>,
) -> Result<Vec<InstalledApp>, AppError> {
    store.list()
}

/// Install (or update) an app: download the `.vibox`, unpack it into the
/// shared repos dir, and `podman load` the image inside the VM. The host
/// port assignment survives updates; the data volume is version-independent.
#[tauri::command]
#[specta::specta]
pub async fn vibox_install_app(
    store: State<'_, Arc<AppStore>>,
    vm: State<'_, Arc<VmLifecycle>>,
    slug: String,
) -> Result<InstalledApp, AppError> {
    validate_slug(&slug)?;
    let (fetched, download_url) = registry::fetch_latest(&slug).await?;
    ensure_slug_matches(&slug, &fetched.slug)?;

    let dir = paths::repos_dir()?.join(&slug);
    tokio::fs::create_dir_all(&dir).await?;
    let archive = dir.join("app.vibox");
    registry::download_vibox(&download_url, &archive).await?;

    let manifest = {
        let archive = archive.clone();
        let dir = dir.clone();
        tokio::task::spawn_blocking(move || registry::unpack_vibox(&archive, &dir)).await??
    };
    let _ = tokio::fs::remove_file(&archive).await;
    ensure_slug_matches(&slug, &manifest.slug)?;
    manifest.validate()?;

    let load_cmd = format!("podman load -i /repos/{slug}/image.tar");
    let (code, _stdout, stderr) = vm.exec_host(&load_cmd).await?;
    if code != 0 {
        return Err(AppError::ContainerFailed {
            reason: format!("podman load failed for '{slug}' (exit {code}): {stderr}"),
        });
    }

    // Keep a previously assigned host port so the app reopens on the same URL.
    let host_port = store.get(&slug)?.and_then(|existing| existing.host_port);
    let app = InstalledApp {
        manifest,
        host_port,
        running: false,
    };
    store.upsert(app.clone())?;
    Ok(app)
}

/// Run an installed app and return its host port.
#[tauri::command]
#[specta::specta]
pub async fn vibox_run_app(
    store: State<'_, Arc<AppStore>>,
    vm: State<'_, Arc<VmLifecycle>>,
    forwards: State<'_, Arc<AppForwards>>,
    slug: String,
) -> Result<u16, AppError> {
    run_app_inner(&store, &vm, &forwards, &slug).await
}

/// Shared run path for [`vibox_run_app`] and the startup auto-run hook.
///
/// `podman rm -f` before `run` makes this idempotent: re-running an already
/// running app (or one whose old container exited) recreates the container
/// from the freshest loaded image, with `/data` persisting in the named
/// volume.
pub async fn run_app_inner(
    store: &AppStore,
    vm: &VmLifecycle,
    forwards: &AppForwards,
    slug: &str,
) -> Result<u16, AppError> {
    validate_slug(slug)?;
    let app = store.get(slug)?.ok_or_else(|| not_installed(slug))?;
    // Re-validate the stored manifest: its `version` reaches the image ref in
    // the shell command below, so never trust it just because it was persisted.
    app.manifest.validate()?;
    let port = store.allocate_port(slug)?;
    let manifest = &app.manifest;

    let run_cmd = format!(
        "podman volume create {vol} >/dev/null 2>&1; podman rm -f {name} >/dev/null 2>&1; \
         podman run -d --name {name} -p {port}:{internal} -v {vol}:/data {image}",
        vol = manifest.volume_name(),
        name = manifest.container_name(),
        internal = manifest.internal_port,
        image = manifest.image_ref(),
    );
    let (code, _stdout, stderr) = vm.exec_host(&run_cmd).await?;
    if code != 0 {
        return Err(AppError::ContainerFailed {
            reason: format!("podman run failed for '{slug}' (exit {code}): {stderr}"),
        });
    }

    // Replace any existing forwarder before binding the host port again.
    let vsock_path = vm.vsock_path().await.ok_or(AppError::VmNotRunning)?;
    if let Some(old) = forwards.0.lock().await.remove(slug) {
        crate::vm::port_forward::stop_forwarding(&old);
    }
    let handle = crate::vm::port_forward::start_forwarding(&vsock_path, port, port);
    forwards.0.lock().await.insert(slug.to_string(), handle);

    store.set_running(slug, true)?;
    Ok(port)
}

/// Stop a running app's container and tear down its port forward.
#[tauri::command]
#[specta::specta]
pub async fn vibox_stop_app(
    store: State<'_, Arc<AppStore>>,
    vm: State<'_, Arc<VmLifecycle>>,
    forwards: State<'_, Arc<AppForwards>>,
    slug: String,
) -> Result<(), AppError> {
    validate_slug(&slug)?;
    let app = store.get(&slug)?.ok_or_else(|| not_installed(&slug))?;

    let stop_cmd = format!("podman stop -t 5 {}", app.manifest.container_name());
    let (code, _stdout, stderr) = vm.exec_host(&stop_cmd).await?;
    if code != 0 {
        // Container already gone is not a failure worth surfacing; the
        // store flag below is what the launcher renders.
        tracing::warn!("podman stop for '{slug}' exited {code}: {stderr}");
    }

    if let Some(handle) = forwards.0.lock().await.remove(&slug) {
        crate::vm::port_forward::stop_forwarding(&handle);
    }
    store.set_running(&slug, false)?;
    Ok(())
}

/// Open (or focus) the app's dedicated window pointing at its host port.
#[tauri::command]
#[specta::specta]
pub async fn vibox_open_app(
    app: tauri::AppHandle,
    store: State<'_, Arc<AppStore>>,
    slug: String,
) -> Result<(), AppError> {
    validate_slug(&slug)?;
    let installed = store.get(&slug)?.ok_or_else(|| not_installed(&slug))?;
    let port = installed.host_port.ok_or_else(|| AppError::InvalidInput {
        field: "slug".into(),
        reason: format!("app '{slug}' has no host port yet; run it first"),
    })?;

    let label = format!("app-{slug}");
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    let url = format!("http://localhost:{port}")
        .parse()
        .map_err(|e| AppError::Internal {
            reason: format!("cannot parse app url for '{slug}': {e}"),
        })?;
    let window = tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::External(url))
        .title(&installed.manifest.name)
        .inner_size(1100.0, 750.0)
        .build()?;
    let _ = window.show();
    let _ = window.set_focus();
    Ok(())
}

/// Run every installed app, logging and continuing on individual failures so
/// one broken app never blocks the rest. Called after the VM reaches Running
/// on startup.
pub async fn auto_start_installed(store: &AppStore, vm: &VmLifecycle, forwards: &AppForwards) {
    let apps = match store.list() {
        Ok(apps) => apps,
        Err(e) => {
            tracing::error!("cannot list installed apps for auto-start: {e}");
            return;
        }
    };
    for app in apps {
        let slug = app.manifest.slug;
        match run_app_inner(store, vm, forwards, &slug).await {
            Ok(port) => tracing::info!("auto-started app '{slug}' on port {port}"),
            Err(e) => tracing::error!("cannot auto-start app '{slug}': {e}"),
        }
    }
}

fn ensure_slug_matches(expected: &str, actual: &str) -> Result<(), AppError> {
    if expected == actual {
        Ok(())
    } else {
        Err(AppError::InvalidInput {
            field: "slug".into(),
            reason: format!("manifest slug '{actual}' does not match requested '{expected}'"),
        })
    }
}
