//! Constructs the Tauri builder, wires shared state, and runs the app.
//!
//! Re-exported as `run` from `app/mod.rs`. Per-platform behavior is
//! dispatched through `PlatformBootstrap` and `register::run_*`.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use super::platform::PlatformBootstrap;
use super::register;
use crate::auth::{Authenticator, GitHubReposClient, KeychainTokenStore};
use crate::constants::events;
use crate::error::AppError;
use crate::infrastructure::{GitOps, LibGitClient, OsKeychain, RecoveryReport, Runtime};
use crate::projects::runner::RunningContainers;
use crate::projects::{JsonProjectStore, ProjectImporter, ProjectStore, ProjectSyncer};
use crate::settings::{JsonSettingsStore, SettingsStore};
use crate::tray;
use crate::tunnel::TunnelCoordinator;

pub fn run() -> Result<(), AppError> {
    // Order matters: panic hook is installed BEFORE the subscriber so a panic
    // during tracing setup still writes to panic.log. The hook also logs via
    // `tracing::error!`, which becomes a no-op until init() finishes; that is
    // acceptable, the file-side capture is the primary trail.
    super::panic_hook::init();
    let _log_guard = super::logging::init()?;

    // Bind the MCP socket before any slower startup I/O so a just-relaunched
    // Opnble starts accepting the editor's connection immediately (the kernel
    // queues it in the listen backlog); serving is wired in `setup` once the
    // dependency graph exists. See `mcp::bind_listener` for why the refuse
    // window is what strands the Cursor client across a restart.
    let mcp_listener = tauri::async_runtime::block_on(crate::mcp::bind_listener());

    // ==================== Platform Bootstrap ====================
    #[cfg(target_os = "macos")]
    let platform: Arc<dyn PlatformBootstrap> = Arc::new(super::platform::macos::MacosBoot::new());
    #[cfg(target_os = "linux")]
    let platform: Arc<dyn PlatformBootstrap> = Arc::new(super::platform::linux::LinuxBoot);
    #[cfg(target_os = "windows")]
    let platform: Arc<dyn PlatformBootstrap> =
        Arc::new(super::platform::windows::WindowsBoot::new());

    let runtime: Arc<dyn Runtime> = platform.runtime();
    let exec_runner: Arc<dyn crate::mcp::exec::ExecRunner> = platform.exec_runner();

    // ==================== Infrastructure Layer ====================
    let git: Arc<dyn GitOps> = Arc::new(LibGitClient::new());

    // ==================== Stores ====================
    let (project_store_inner, project_recovery) = JsonProjectStore::load_or_default()?;
    let (settings_store_inner, settings_recovery) = JsonSettingsStore::load_or_default()?;
    let project_store: Arc<dyn ProjectStore> = Arc::new(project_store_inner);
    let settings_store: Arc<dyn SettingsStore> = Arc::new(settings_store_inner);

    let recovery_reports: Vec<(&'static str, RecoveryReport)> = [
        project_recovery.map(|r| ("projects", r)),
        settings_recovery.map(|r| ("settings", r)),
    ]
    .into_iter()
    .flatten()
    .collect();

    // ==================== Tunnel Sharing ====================
    let tunnel_coordinator = Arc::new(TunnelCoordinator::new(std::path::PathBuf::from(
        crate::constants::tunnel::CLOUDFLARED_BINARY,
    ))?);

    // ==================== Publishing ====================
    let github_pages_client = Arc::new(crate::publish::github_pages::GitHubPagesClient::new()?);
    let github_repos_client = Arc::new(GitHubReposClient::new()?);

    // ==================== Tray State ====================
    let tray_state = Arc::new(tray::TrayState::new());

    // ==================== Dependencies ====================
    let running = Arc::new(RunningContainers::new());
    let orchestrator = Arc::new(crate::projects::ProjectOrchestrator::new(
        Arc::clone(&runtime),
        Arc::clone(&project_store),
        Some(Arc::clone(&tunnel_coordinator)),
    ));
    let importer = Arc::new(ProjectImporter::new(
        Arc::clone(&project_store),
        Arc::clone(&git),
    ));
    let authenticator = Arc::new(Authenticator::new(Arc::new(KeychainTokenStore::new(
        Arc::new(OsKeychain::new()),
    ))));
    let syncer = Arc::new(ProjectSyncer::new(
        Arc::clone(&project_store),
        Arc::clone(&settings_store),
        Arc::clone(&git),
        Arc::clone(&authenticator),
    ));

    // Recent-MCP-tool-invocation ring. Constructed here so it lives in
    // Tauri-managed state (the `mcp_recent_tool_invocations` command pulls
    // the same Arc that `McpDeps::collect` hands to `OpnbleMcp`).
    let tool_ring = Arc::new(crate::mcp::ring::ToolRing::with_default_cap());

    // ==================== App Builder ====================
    let platform_for_event = Arc::clone(&platform);
    let platform_for_setup = Arc::clone(&platform);
    let orchestrator_for_setup = Arc::clone(&orchestrator);

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .on_window_event(move |window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                platform_for_event.on_close_requested(window, api);
            }
        })
        .setup(move |app| {
            // macOS: hide from dock until a window is shown
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            if let Err(e) = tray::create_tray(app.handle()) {
                tracing::error!("cannot create system tray: {e}");
            }

            resolve_cloudflared(app);

            spawn_mcp_listener(app.handle(), mcp_listener);

            if let Err(e) = orchestrator_for_setup.mark_all_stopped_in_store() {
                tracing::error!(error = %e, "cannot reset projects on startup");
            }

            if let Err(e) = platform_for_setup.on_setup(app.handle()) {
                tracing::error!("platform setup failed: {e}");
            }

            emit_recovery_reports(app.handle(), &recovery_reports);

            crate::app::updater::schedule_periodic_check(app.handle().clone());

            Ok(())
        })
        // Manage tray state
        .manage(Arc::clone(&tray_state))
        // Manage infrastructure
        .manage(Arc::clone(&runtime))
        .manage(Arc::clone(&git))
        // Manage stores
        .manage(Arc::clone(&project_store))
        .manage(Arc::clone(&settings_store))
        // Manage services
        .manage(Arc::clone(&importer))
        .manage(Arc::clone(&authenticator))
        .manage(Arc::clone(&running))
        .manage(Arc::clone(&orchestrator))
        .manage(Arc::clone(&syncer))
        .manage(Arc::clone(&tunnel_coordinator))
        .manage(github_pages_client)
        .manage(github_repos_client)
        .manage(Arc::clone(&exec_runner))
        .manage(Arc::clone(&tool_ring));

    // Platform-specific state management
    builder = platform.manage_state(builder);

    // Register all commands and run app.
    #[cfg(target_os = "macos")]
    register::run_macos(builder);
    #[cfg(target_os = "windows")]
    register::run_windows(builder);
    #[cfg(target_os = "linux")]
    register::run_linux(builder);

    Ok(())
}

fn emit_recovery_reports(app: &AppHandle, reports: &[(&'static str, RecoveryReport)]) {
    for (scope, report) in reports {
        let payload = match report {
            RecoveryReport::RecoveredFromBackup => serde_json::json!({
                "scope": scope,
                "kind": "recoveredFromBackup",
            }),
            RecoveryReport::ArchivedAndReset { archive_paths } => serde_json::json!({
                "scope": scope,
                "kind": "archivedAndReset",
                "archivePaths": archive_paths,
            }),
        };
        if let Err(e) = app.emit(events::state_recovery(), payload) {
            tracing::error!("cannot emit state-recovery event: {e}");
        }
    }
}

/// Start the local MCP listener so coding agents can reach Opnble.
///
/// Mirrors `resolve_cloudflared`'s "spawn then forget" shape: errors are
/// logged but never bubble out of `setup`, so the rest of the app still
/// launches even if the listener cannot bind (port held, missing config,
/// etc). On success, registers the listener's [`crate::mcp::auth::SharedToken`]
/// as Tauri-managed state so `mcp_rotate_token` can update the live token
/// in lockstep with the on-disk file (without it, rotation would silently
/// leave the old in-memory token authoritative).
///
/// Also deep-merges `mcpServers.opnble` into `~/.cursor/mcp.json` so the
/// Opnble tool surface shows up in every Cursor window without a user
/// action, not only in folders Opnble imported. The global write is
/// best-effort and silently skipped on machines without Cursor installed
/// (no `~/.cursor/` directory).
fn spawn_mcp_listener(app: &tauri::AppHandle, listener: Option<tokio::net::TcpListener>) {
    let Some(listener) = listener else {
        tracing::error!(
            "mcp listener not bound (port held or bind failed); agent control disabled"
        );
        return;
    };
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let deps = crate::mcp::McpDeps::collect(&app_handle);
        match crate::mcp::spawn(deps, listener).await {
            Ok(spawned) => {
                let token_snapshot = spawned.token.read().await.clone();
                app_handle.manage(Arc::clone(&spawned.token));
                app_handle.manage(crate::mcp::McpHandle::new(spawned));
                tracing::info!("mcp listener ready at {}", crate::mcp::config::mcp_url());
                install_global_cursor_mcp(&token_snapshot);
            }
            Err(e) => tracing::error!("cannot start mcp listener: {e}"),
        }
    });
}

/// Deep-merge `mcpServers.opnble` into `~/.cursor/mcp.json` so every
/// Cursor window sees the local Opnble MCP server. Best-effort: a missing
/// `~/.cursor/` directory means Cursor is not installed and we skip
/// silently; an unwritable file is logged but does not block startup
/// (the per-project install still works for imported projects).
fn install_global_cursor_mcp(token: &str) {
    use crate::mcp::install::{write_global_mcp_config, GlobalInstall};
    match write_global_mcp_config(token, crate::mcp::config::PORT) {
        Ok(GlobalInstall::Written { path }) => {
            tracing::info!(
                path = %path.display(),
                "installed opnble entry into global cursor mcp.json"
            );
        }
        Ok(GlobalInstall::CursorNotDetected) => {
            tracing::debug!("cursor not detected on this machine; skipping global mcp install");
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "cannot install opnble entry into global cursor mcp.json; \
                 per-project install still works for imported projects"
            );
        }
    }
}

fn resolve_cloudflared(app: &tauri::App) {
    let handle = app.handle().clone();
    let Some(bundled) = crate::tunnel::bundled_cloudflared_path(&handle) else {
        tracing::debug!("cloudflared: bundled sidecar not present, falling back to PATH");
        return;
    };

    let tm = app.state::<Arc<TunnelCoordinator>>();
    let tm = Arc::clone(tm.inner());
    tauri::async_runtime::spawn(async move {
        tm.set_cloudflared_path(bundled).await;
    });
    tracing::info!("cloudflared: using bundled sidecar binary");
}
