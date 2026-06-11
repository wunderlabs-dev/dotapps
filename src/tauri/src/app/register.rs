/// Builds a `tauri_specta::Builder` with shared commands (projects, tunnel,
/// settings, auth, window) plus the platform-specific commands passed in.
///
/// Each platform provides native implementations of VM/setup commands and
/// stubs for commands belonging to other platforms.
macro_rules! specta_builder_with {
    ($($platform_cmd:tt)*) => {
        tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
            // Projects
            crate::projects::commands::state,
            crate::projects::commands::list_projects,
            crate::projects::commands::project,
            crate::projects::commands::add_project,
            crate::projects::commands::update_project,
            crate::projects::commands::remove_project,
            crate::projects::commands::import_project,
            crate::projects::commands::clone_repository,
            crate::projects::commands::pull_repository,
            crate::projects::commands::checkout_branch,
            crate::projects::commands::list_branches,
            crate::projects::commands::is_cursor_installed,
            crate::projects::commands::open_project_in_cursor,
            crate::projects::commands::install_project,
            crate::projects::commands::start_project,
            crate::projects::commands::stop_project,
            crate::projects::commands::restart_project,
            // Tunnel sharing
            crate::tunnel::commands::share_project,
            crate::tunnel::commands::unshare_project,
            // Publishing
            crate::publish::commands::publish_project,
            crate::publish::commands::unpublish_project,
            // MCP (Cursor integration)
            crate::mcp::commands::mcp_status,
            crate::mcp::commands::mcp_rotate_token,
            crate::mcp::commands::mcp_list_snapshots,
            crate::mcp::commands::mcp_delete_snapshot,
            crate::mcp::commands::mcp_discard_fork,
            crate::mcp::commands::mcp_rollback_project,
            crate::mcp::commands::mcp_recent_tool_invocations,
            // Settings
            crate::settings::commands::settings,
            crate::settings::commands::save_settings,
            // Auth
            crate::auth::commands::store_auth_token,
            crate::auth::commands::auth_token,
            crate::auth::commands::delete_auth_token,
            crate::auth::commands::provider_from_url,
            crate::auth::commands::github_start_oauth,
            crate::auth::commands::github_poll_oauth,
            crate::auth::commands::github_user,
            crate::auth::commands::github_list_repos,
            crate::auth::commands::check_provider_support,
            crate::auth::commands::validate_auth_token,
            // Window
            crate::app::window_commands::show_window,
            crate::app::window_commands::quit_app,
            // Integrity
            crate::app::integrity::check_data_integrity,
            // Updater
            crate::app::updater::check_for_update,
            crate::app::updater::install_update,
            // Diagnostics + version (Phase 3 beta tooling)
            crate::diagnostics::commands::app_version,
            crate::diagnostics::commands::export_diagnostics,
            crate::diagnostics::commands::reveal_logs_folder,
            // Platform-specific
            $($platform_cmd)*
        ])
        // Event payload emitted via the raw `vm-image-progress` string, so it is
        // not a command type; register it explicitly so the TS type is exported.
        .typ::<crate::vm::VmImageProgress>()
    };
}

/// Build the macOS-platform specta builder. Shared by `run_macos` and the
/// `regen_bindings` example so we can regenerate TypeScript bindings without
/// spinning up the Tauri runtime.
#[cfg(target_os = "macos")]
pub fn macos_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    specta_builder_with![
        // Vibox apps (macOS native: podman runs via the VM agent)
        crate::apps::commands::vibox_registry_apps,
        crate::apps::commands::vibox_installed_apps,
        crate::apps::commands::vibox_install_app,
        crate::apps::commands::vibox_run_app,
        crate::apps::commands::vibox_stop_app,
        crate::apps::commands::vibox_open_app,
        // VM (macOS native)
        crate::app::platform::macos::init_vm,
        crate::app::platform::macos::stop_vm,
        crate::app::platform::macos::is_vm_running,
        crate::app::platform::macos::vm_status,
        crate::app::platform::macos::container_status,
        crate::app::platform::macos::list_running_containers,
        crate::app::platform::macos::vm_stats,
        crate::app::platform::macos::vm_exec,
        // VM image downloader (macOS native; Phase 2)
        crate::app::platform::macos::vm_image_status,
        crate::app::platform::macos::download_vm_image,
        crate::app::platform::macos::cancel_vm_image_download,
        // Setup (macOS stubs for Windows/Linux commands)
        crate::app::platform::macos::check_setup_status,
        crate::app::platform::macos::run_initial_setup,
        crate::app::platform::macos::check_windows_setup,
        crate::app::platform::macos::enable_wsl_windows,
        crate::app::platform::macos::init_wsl,
        crate::app::platform::macos::reboot_windows,
    ]
}

/// Default TypeScript export config used by both the debug-build auto-export
/// and the `regen_bindings` example.
pub fn typescript_export_config() -> specta_typescript::Typescript {
    specta_typescript::Typescript::default()
        .bigint(specta_typescript::BigIntExportBehavior::Number)
        .header("// @ts-nocheck")
}

#[cfg(target_os = "macos")]
pub fn run_macos(builder: tauri::Builder<tauri::Wry>) {
    let specta_builder = macos_specta_builder();

    #[cfg(debug_assertions)]
    specta_builder
        .export(typescript_export_config(), "../web/gen/tauri.ts")
        .expect("failed to export typescript bindings");

    // No .setup() call needed here: we register zero events, so mount_events is a no-op.
    // The existing .setup() from bootstrap.rs (tray creation, VM auto-start) is preserved.
    // When events are added, merge mount_events into bootstrap's setup closure.
    run_with_graceful_shutdown(builder.invoke_handler(specta_builder.invoke_handler()));
}

/// Build the app and tear down projects, tunnels, and the VM on event-loop exit.
///
/// Catches Cmd+Q and OS-driven quits that never route through the tray's quit
/// item; `cargo tauri dev` SIGKILL on hot rebuild is uncatchable and is handled
/// by pidfile reaping in the VM backend instead.
fn run_with_graceful_shutdown(builder: tauri::Builder<tauri::Wry>) {
    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            tauri::async_runtime::block_on(crate::app::graceful_shutdown(app_handle));
        }
    });
}

#[cfg(target_os = "windows")]
pub fn run_windows(builder: tauri::Builder<tauri::Wry>) {
    let specta_builder = specta_builder_with![
        // Windows (native)
        crate::app::platform::windows::check_windows_setup,
        crate::app::platform::windows::enable_wsl_windows,
        crate::app::platform::windows::init_wsl,
        crate::app::platform::windows::reboot_windows,
        // Windows stubs for macOS/Linux commands
        crate::app::platform::windows::init_vm,
        crate::app::platform::windows::is_vm_running,
        crate::app::platform::windows::vm_status,
        crate::app::platform::windows::container_status,
        crate::app::platform::windows::list_running_containers,
        crate::app::platform::windows::vm_stats,
        crate::app::platform::windows::check_setup_status,
        crate::app::platform::windows::run_initial_setup,
        crate::app::platform::windows::vm_image_status,
        crate::app::platform::windows::download_vm_image,
        crate::app::platform::windows::cancel_vm_image_download,
    ];

    run_with_graceful_shutdown(builder.invoke_handler(specta_builder.invoke_handler()));
}

#[cfg(target_os = "linux")]
pub fn run_linux(builder: tauri::Builder<tauri::Wry>) {
    let specta_builder = specta_builder_with![
        // Linux (native)
        crate::app::platform::linux::is_vm_running,
        crate::app::platform::linux::vm_status,
        crate::app::platform::linux::check_setup_status,
        crate::app::platform::linux::run_initial_setup,
        crate::app::platform::linux::container_status,
        crate::app::platform::linux::list_running_containers,
        crate::app::platform::linux::vm_stats,
        // Linux stubs for macOS/Windows commands
        crate::app::platform::linux::init_vm,
        crate::app::platform::linux::check_windows_setup,
        crate::app::platform::linux::enable_wsl_windows,
        crate::app::platform::linux::init_wsl,
        crate::app::platform::linux::reboot_windows,
        crate::app::platform::linux::vm_image_status,
        crate::app::platform::linux::download_vm_image,
        crate::app::platform::linux::cancel_vm_image_download,
    ];

    run_with_graceful_shutdown(builder.invoke_handler(specta_builder.invoke_handler()));
}
