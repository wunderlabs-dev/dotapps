//! `dotapps://` deep-link handling: install (and launch) an app from a link.
//!
//! `dotapps://{slug}` installs the latest version; `dotapps://{slug}@{version}`
//! installs an exact version. The link is parsed by hand rather than via a URL
//! crate: `dotapps://cafe-tracker@2.0.0` would otherwise be read as
//! `userinfo@host` and split the slug from the version incorrectly.

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::commands::{install_inner, open_inner, run_app_inner};
use super::store::AppStore;
use super::AppForwards;
use crate::error::AppError;
use crate::vm::VmLifecycle;

/// Max time to wait for the VM on a cold launch triggered by a deep link.
const VM_WAIT: Duration = Duration::from_secs(90);

/// Event name the launcher listens on for deep-link install progress.
const PROGRESS_EVENT: &str = "dotapps-install";

/// Progress update emitted as a deep-link install moves through its phases,
/// so the Library can show an "Installing…" tile while it runs.
#[derive(Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub slug: String,
    pub name: Option<String>,
    /// One of: `installing`, `starting`, `ready`, `failed`.
    pub phase: String,
    pub error: Option<String>,
}

fn emit_progress(
    app: &AppHandle,
    slug: &str,
    name: Option<&str>,
    phase: &str,
    error: Option<String>,
) {
    let _ = app.emit(
        PROGRESS_EVENT,
        InstallProgress {
            slug: slug.to_string(),
            name: name.map(str::to_string),
            phase: phase.to_string(),
            error,
        },
    );
}

/// Parsed `dotapps://` target: an app slug and an optional exact version.
#[derive(Debug, PartialEq, Eq)]
pub struct DeepLink {
    pub slug: String,
    pub version: Option<String>,
}

/// Parse a `dotapps://{slug}[@{version}]` URL. Field validation is left to
/// the install path (slug/version are checked there before any shell use).
pub fn parse(url: &str) -> Result<DeepLink, AppError> {
    let rest = url
        .strip_prefix("dotapps://")
        .ok_or_else(|| invalid(format!("not a dotapps:// link: {url}")))?;
    // Tolerate a trailing slash and surrounding whitespace from shells/QR apps.
    let rest = rest.trim().trim_end_matches('/');
    if rest.is_empty() {
        return Err(invalid("link has no app slug".to_string()));
    }
    let (slug, version) = match rest.split_once('@') {
        Some((slug, version)) => (slug, Some(version.to_string())),
        None => (rest, None),
    };
    Ok(DeepLink {
        slug: slug.to_string(),
        version,
    })
}

/// Handle one deep-link URL: wait for the VM, install the requested version,
/// run it, open its window, and surface the launcher. Errors are logged (the
/// link arrives from outside the UI, so there is no caller to return them to).
pub async fn handle(app: AppHandle, url: String) {
    let link = match parse(&url) {
        Ok(link) => link,
        Err(e) => {
            tracing::warn!("ignoring deep link {url:?}: {e}");
            return;
        }
    };
    tracing::info!(slug = %link.slug, version = ?link.version, "handling deep link");

    let store = Arc::clone(app.state::<Arc<AppStore>>().inner());
    let vm = Arc::clone(app.state::<Arc<VmLifecycle>>().inner());
    let forwards = Arc::clone(app.state::<Arc<AppForwards>>().inner());

    surface_launcher(&app);
    emit_progress(&app, &link.slug, None, "installing", None);

    if !vm.wait_until_running(VM_WAIT).await {
        tracing::error!("deep link {url:?}: VM not ready after {VM_WAIT:?}");
        emit_progress(
            &app,
            &link.slug,
            None,
            "failed",
            Some("VM not ready".to_string()),
        );
        return;
    }

    let installed = match install_inner(&store, &vm, &link.slug, link.version.as_deref()).await {
        Ok(installed) => installed,
        Err(e) => {
            tracing::error!("deep link {url:?}: install failed: {e}");
            emit_progress(&app, &link.slug, None, "failed", Some(e.to_string()));
            return;
        }
    };
    let name = installed.manifest.name.clone();

    emit_progress(&app, &link.slug, Some(&name), "starting", None);
    if let Err(e) = run_app_inner(&store, &vm, &forwards, &link.slug).await {
        tracing::error!("deep link {url:?}: run failed: {e}");
        emit_progress(&app, &link.slug, Some(&name), "failed", Some(e.to_string()));
        return;
    }

    emit_progress(&app, &link.slug, Some(&name), "ready", None);
    if let Err(e) = open_inner(&app, &store, &link.slug) {
        tracing::error!("deep link {url:?}: open failed: {e}");
    }
}

/// Show and focus the main launcher window so the deep-link install is visible.
fn surface_launcher(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn invalid(reason: String) -> AppError {
    AppError::InvalidInput {
        field: "url".into(),
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, DeepLink};

    #[test]
    fn parses_slug_and_version() {
        assert_eq!(
            parse("dotapps://cafe-tracker@2.0.0").unwrap(),
            DeepLink {
                slug: "cafe-tracker".to_string(),
                version: Some("2.0.0".to_string()),
            }
        );
    }

    #[test]
    fn parses_bare_slug_as_latest() {
        assert_eq!(
            parse("dotapps://cafe-tracker").unwrap(),
            DeepLink {
                slug: "cafe-tracker".to_string(),
                version: None,
            }
        );
    }

    #[test]
    fn tolerates_trailing_slash_and_whitespace() {
        assert_eq!(
            parse("dotapps://shift-board/ ").unwrap(),
            DeepLink {
                slug: "shift-board".to_string(),
                version: None,
            }
        );
    }

    #[test]
    fn rejects_foreign_scheme_and_empty_slug() {
        assert!(parse("https://example.com").is_err());
        assert!(parse("dotapps://").is_err());
    }
}
