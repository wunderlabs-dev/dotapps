//! System tray management for Opnble
//!
//! Handles the menu bar icon and dropdown menu.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Emitter, Manager, Wry,
};

use crate::app::graceful_shutdown;
use crate::error::AppError;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::vm::VmLifecycle;

pub const STATUS_STOPPED: u8 = 0;
pub const STATUS_STARTING: u8 = 1;
pub const STATUS_RUNNING: u8 = 2;
pub const STATUS_STOPPING: u8 = 3;

/// VM status tracked as Tauri managed state instead of a module-level static.
///
/// Uses `AtomicU8` for lock-free reads from the main thread (tray menu rebuild)
/// and writes from async tasks (VM lifecycle callbacks).
pub struct TrayState {
    status: AtomicU8,
}

impl TrayState {
    pub fn new() -> Self {
        Self {
            status: AtomicU8::new(STATUS_STOPPED),
        }
    }

    pub fn load(&self) -> u8 {
        self.status.load(Ordering::SeqCst)
    }

    pub fn store(&self, value: u8) {
        self.status.store(value, Ordering::SeqCst);
    }
}

/// Retrieve the `TrayState` from managed Tauri state.
fn tray_state(app: &AppHandle) -> Arc<TrayState> {
    Arc::clone(app.state::<Arc<TrayState>>().inner())
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn vm_status_label(state: &TrayState) -> &'static str {
    match state.load() {
        STATUS_STARTING => "VM: Starting",
        STATUS_RUNNING => "VM: Running",
        STATUS_STOPPING => "VM: Stopping",
        _ => "VM: Stopped",
    }
}

fn vm_status_icon(state: &TrayState) -> &'static str {
    match state.load() {
        STATUS_STARTING | STATUS_STOPPING => "icons/tray-icon-starting.png",
        STATUS_RUNNING => "icons/tray-icon-running.png",
        _ => "icons/tray-icon-stopped.png",
    }
}

/// Check if the VM is currently running.
pub fn is_vm_running(app: &AppHandle) -> bool {
    tray_state(app).load() == STATUS_RUNNING
}

/// Update the tray to reflect current VM status.
///
/// Safe to call from any thread: the atomic status is updated immediately,
/// then tray operations (`set_icon`, `set_menu`) are dispatched to the
/// main thread to avoid crashes from background-thread access.
pub fn update_tray_status(app: &AppHandle, status: u8) {
    tray_state(app).store(status);

    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let state = tray_state(&handle);

        let Some(tray) = handle.tray_by_id("main") else {
            tracing::error!("cannot find tray with id 'main'");
            return;
        };

        match load_tray_icon(&handle, vm_status_icon(&state)) {
            Ok(icon) => {
                let _ = tray.set_icon(Some(icon));
            }
            Err(e) => {
                tracing::error!("cannot load tray icon: {e}");
            }
        }

        if let Ok(menu) = build_tray_menu(&handle) {
            let _ = tray.set_menu(Some(menu));
        }
    });
}

/// Update the tray icon based on VM running status.
pub fn update_tray_icon(app: &AppHandle, running: bool) {
    let status = if running {
        STATUS_RUNNING
    } else {
        STATUS_STOPPED
    };
    update_tray_status(app, status);
}

/// Activate the macOS app so tray menu events are delivered.
///
/// `LSUIElement` apps start with `Prohibited` activation policy, which can
/// cause macOS to swallow the first menu-item click while activating the
/// process. Calling this before (or during) menu interaction ensures the
/// app is already active.
#[cfg(target_os = "macos")]
#[expect(
    unsafe_code,
    reason = "objc FFI to activate background app so macOS delivers tray menu events"
)]
fn activate_macos_app() {
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let ns_app: *mut objc::runtime::Object =
            msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
    }
}

/// Switch to `Regular` activation policy so the app appears in Dock and Cmd+Tab.
///
/// Called when the main application window is opened.
/// Paired with `hide_dock_icon` when all windows close.
#[cfg(target_os = "macos")]
#[expect(
    unsafe_code,
    reason = "objc FFI to toggle macOS activation policy for Dock visibility"
)]
pub fn show_dock_icon() {
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let ns_app: *mut objc::runtime::Object =
            msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![ns_app, setActivationPolicy: 0_isize];
        let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
    }
}

/// Switch to `Accessory` activation policy so the app hides from Dock and Cmd+Tab.
///
/// Called when the last visible window is closed, returning to tray-only mode.
#[cfg(target_os = "macos")]
#[expect(
    unsafe_code,
    reason = "objc FFI to toggle macOS activation policy for Dock visibility"
)]
pub fn hide_dock_icon() {
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let ns_app: *mut objc::runtime::Object =
            msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![ns_app, setActivationPolicy: 1_isize];
    }
}

/// Create the system tray icon with menu and event handlers.
pub fn create_tray(app: &AppHandle) -> Result<TrayIcon, AppError> {
    let menu = build_tray_menu(app)?;
    let icon = load_tray_icon(app, "icons/tray-icon.png")?;

    let tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            handle_menu_event(app, event.id().as_ref());
        })
        .build(app)?;

    Ok(tray)
}

/// Build the tray dropdown menu.
///
/// On macOS/Windows the menu includes VM status and start/stop controls.
/// On Linux (native Podman, no VM) those items are omitted.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn build_tray_menu(app: &AppHandle) -> Result<Menu<Wry>, AppError> {
    let state = tray_state(app);
    let status = state.load();
    let status_item = MenuItem::with_id(
        app,
        "vm_status",
        vm_status_label(&state),
        false,
        None::<&str>,
    )?;

    let vm_toggle = match status {
        STATUS_RUNNING => MenuItem::with_id(app, "vm_stop", "Stop VM", true, None::<&str>)?,
        STATUS_STOPPED => MenuItem::with_id(app, "vm_start", "Start VM", true, None::<&str>)?,
        _ => MenuItem::with_id(
            app,
            "vm_transition",
            if status == STATUS_STARTING {
                "Starting..."
            } else {
                "Stopping..."
            },
            false,
            None::<&str>,
        )?,
    };

    let separator1 = PredefinedMenuItem::separator(app)?;
    let dashboard = MenuItem::with_id(app, "dashboard", "Show Dashboard", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Ok(Menu::with_items(
        app,
        &[
            &status_item,
            &vm_toggle,
            &separator1,
            &dashboard,
            &settings,
            &separator2,
            &quit,
        ],
    )?)
}

/// Build the tray dropdown menu.
///
/// On Linux (native Podman, no VM) the VM status and start/stop items are omitted.
#[cfg(target_os = "linux")]
fn build_tray_menu(app: &AppHandle) -> Result<Menu<Wry>, AppError> {
    let dashboard = MenuItem::with_id(app, "dashboard", "Show Dashboard", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    Ok(Menu::with_items(
        app,
        &[&dashboard, &settings, &separator, &quit],
    )?)
}

/// Handle menu item click events.
fn handle_menu_event(app: &AppHandle, id: &str) {
    #[cfg(target_os = "macos")]
    activate_macos_app();

    match id {
        "dashboard" => show_main_window(app),
        "settings" => {
            show_main_window(app);
            emit_navigate(app, "/settings");
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        "vm_start" => handle_vm_start(app),
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        "vm_stop" => handle_vm_stop(app),
        "quit" => {
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                graceful_shutdown(&app_handle).await;
                app_handle.exit(0);
            });
        }
        _ => {}
    }
}

fn show_main_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    show_dock_icon();

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        match tauri::WebviewWindowBuilder::new(
            app,
            "main",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .title("Opnble")
        .inner_size(1400.0, 900.0)
        .build()
        {
            Ok(window) => {
                let _ = window.show();
                let _ = window.set_focus();
            }
            Err(e) => {
                tracing::error!("cannot create main window: {e}");
            }
        }
    }
}

fn emit_navigate(app: &AppHandle, path: &str) {
    let _ = app.emit("navigate", path);
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn handle_vm_start(app: &AppHandle) {
    update_tray_status(app, STATUS_STARTING);
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        let lifecycle = app_handle.state::<Arc<VmLifecycle>>();
        let lifecycle = Arc::clone(lifecycle.inner());
        match lifecycle.start(&app_handle).await {
            Ok(()) => {
                update_tray_status(&app_handle, STATUS_RUNNING);
                tracing::info!("VM started via tray menu");
            }
            Err(e) => {
                update_tray_status(&app_handle, STATUS_STOPPED);
                tracing::error!("cannot start VM: {e}");
            }
        }
    });
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn handle_vm_stop(app: &AppHandle) {
    update_tray_status(app, STATUS_STOPPING);
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        crate::app::shutdown::stop_all_projects(&app_handle).await;

        let lifecycle = app_handle.state::<Arc<VmLifecycle>>();
        let lifecycle = Arc::clone(lifecycle.inner());
        match lifecycle.stop().await {
            Ok(()) => {
                update_tray_status(&app_handle, STATUS_STOPPED);
                tracing::info!("VM stopped via tray menu");
            }
            Err(e) => {
                update_tray_status(&app_handle, STATUS_RUNNING);
                tracing::error!("cannot stop VM: {e}");
            }
        }
    });
}

/// Load a tray icon from the app resources.
fn load_tray_icon(app: &AppHandle, path: &str) -> Result<Image<'static>, AppError> {
    // Try to load from the resource path first
    let resource_path = app.path().resource_dir()?.join(path);
    if resource_path.exists() {
        return Ok(Image::from_path(resource_path)?);
    }

    // Fall back to loading relative to the executable
    let exe_dir = std::env::current_exe()?
        .parent()
        .ok_or_else(|| AppError::Internal {
            reason: "cannot resolve tray icon: executable has no parent directory".into(),
        })?
        .to_path_buf();
    let icon_path = exe_dir.join(path);
    if icon_path.exists() {
        return Ok(Image::from_path(icon_path)?);
    }

    // For development, try the src/tauri directory
    let dev_path = std::path::Path::new("src/tauri").join(path);
    if dev_path.exists() {
        return Ok(Image::from_path(dev_path)?);
    }

    // Last resort: try direct path
    Ok(Image::from_path(path)?)
}
