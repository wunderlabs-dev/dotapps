use tauri::Manager;

use super::shutdown::graceful_shutdown;
use crate::error::AppError;

/// Show or focus the main application window.
#[tauri::command]
#[specta::specta]
pub async fn show_window(app: tauri::AppHandle) -> Result<(), AppError> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.set_focus()?;
    } else {
        tauri::WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::App("index.html".into()))
            .title("dotapps")
            .inner_size(1400.0, 900.0)
            .build()?;
    }
    Ok(())
}

/// Quit the application with graceful shutdown.
#[tauri::command]
#[specta::specta]
pub async fn quit_app(app: tauri::AppHandle) -> Result<(), AppError> {
    graceful_shutdown(&app).await;
    app.exit(0);
    Ok(())
}
