//! Application error types
//!
//! Provides structured errors that serialize to JSON for the frontend.

use serde::Serialize;
use thiserror::Error;

/// Application error type with structured variants for frontend matching
#[derive(Clone, Debug, Error, Serialize, specta::Type)]
#[serde(tag = "code", content = "detail")]
pub enum AppError {
    // Domain errors
    #[error("cannot find {entity} '{id}'")]
    NotFound { entity: String, id: String },
    #[error("cannot create {entity} '{id}': already exists")]
    AlreadyExists { entity: String, id: String },
    #[error("cannot validate '{field}': {reason}")]
    InvalidInput { field: String, reason: String },

    // Auth errors
    #[error("cannot authenticate with {provider}: login required")]
    AuthRequired { provider: String },
    #[error("cannot authenticate: {reason}")]
    AuthFailed { reason: String },

    // Infrastructure errors
    #[error("cannot reach container runtime: {reason}")]
    ContainerFailed { reason: String },
    #[error("cannot complete git operation: {reason}")]
    GitFailed { reason: String },
    #[error("cannot access storage: {reason}")]
    StorageFailed { reason: String },

    // VM errors
    #[error("cannot reach vm: not running")]
    VmNotRunning,
    #[error("cannot start vm: {reason}")]
    VmStartFailed { reason: String },
    #[error("cannot stop vm: {reason}")]
    VmStopFailed { reason: String },
    #[error("cannot connect to vm: {reason}")]
    VmConnectionFailed { reason: String },
    #[error("cannot set up vm: {reason}")]
    VmSetupFailed { reason: String },

    // Tunnel errors
    #[error("cannot manage tunnel: {reason}")]
    TunnelFailed { reason: String },

    // Publish errors
    #[error("cannot publish project: {0}")]
    Publish(#[from] crate::publish::PublishError),

    // Catch-all
    #[error("{reason}")]
    Internal { reason: String },
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        Self::Internal {
            reason: format_with_source_chain("tauri", &e),
        }
    }
}

/// Format the given error with its full source chain.
///
/// Used by `From` impls that wrap third-party error types so the `Internal`
/// variant carries actionable context instead of a flat top-level string.
fn format_with_source_chain(label: &str, e: &dyn std::error::Error) -> String {
    let mut message = format!("{label}: {e}");
    let mut current = std::error::Error::source(e);
    while let Some(source) = current {
        use std::fmt::Write as _;
        let _ = write!(message, " (caused by: {source})");
        current = source.source();
    }
    message
}

impl From<crate::projects::types::ProjectIdError> for AppError {
    fn from(e: crate::projects::types::ProjectIdError) -> Self {
        use crate::projects::types::ProjectIdError;

        let reason = match e {
            ProjectIdError::Empty => "must not be empty".to_string(),
            ProjectIdError::InvalidCharacters => {
                "contains invalid characters (only alphanumeric and dash allowed)".to_string()
            }
        };
        Self::InvalidInput {
            field: "projectId".into(),
            reason,
        }
    }
}

impl<T> From<std::sync::PoisonError<T>> for AppError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        Self::Internal {
            reason: "lock poisoned".into(),
        }
    }
}

// Convenience conversions from external error types

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::StorageFailed {
            reason: format!("{e} (kind: {:?})", e.kind()),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::StorageFailed {
            reason: format!("{e} (line {}, column {})", e.line(), e.column()),
        }
    }
}

impl From<git2::Error> for AppError {
    fn from(e: git2::Error) -> Self {
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "git2::ErrorClass has 20+ variants; we only categorize the common ones"
        )]
        let class = match e.class() {
            git2::ErrorClass::Net => "network",
            git2::ErrorClass::Ssh => "ssh",
            git2::ErrorClass::Http => "http",
            git2::ErrorClass::Reference => "reference",
            git2::ErrorClass::Repository => "repository",
            git2::ErrorClass::Config => "config",
            _ => "git",
        };
        Self::GitFailed {
            reason: format!("{e} ({class})"),
        }
    }
}

#[cfg(target_os = "linux")]
impl From<bollard::errors::Error> for AppError {
    fn from(e: bollard::errors::Error) -> Self {
        Self::ContainerFailed {
            reason: format_with_source_chain("container runtime", &e),
        }
    }
}

impl From<ttrpc::Error> for AppError {
    fn from(e: ttrpc::Error) -> Self {
        Self::ContainerFailed {
            reason: format_with_source_chain("agent rpc", &e),
        }
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(e: tokio::task::JoinError) -> Self {
        let kind = if e.is_cancelled() {
            "cancelled"
        } else if e.is_panic() {
            "panicked"
        } else {
            "failed"
        };
        Self::Internal {
            reason: format!("cannot join task: {e} ({kind})"),
        }
    }
}

impl From<tauri_plugin_updater::Error> for AppError {
    fn from(e: tauri_plugin_updater::Error) -> Self {
        Self::Internal {
            reason: format_with_source_chain("updater", &e),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        let context = if let Some(status) = e.status() {
            format!(" (HTTP {status})")
        } else if e.is_timeout() {
            " (timeout)".to_string()
        } else if e.is_connect() {
            " (connection failed)".to_string()
        } else {
            String::new()
        };
        Self::Internal {
            reason: format!("{e}{context}"),
        }
    }
}

impl From<crate::vm::VmError> for AppError {
    fn from(err: crate::vm::VmError) -> Self {
        use crate::vm::VmError;
        match err {
            VmError::LaunchFailed { reason } => Self::VmStartFailed { reason },
            VmError::ShutdownFailed { reason } => Self::VmStopFailed { reason },
            VmError::ConnectionFailed { reason } => Self::VmConnectionFailed { reason },
            VmError::ImageNotFound { path } => Self::VmStartFailed {
                reason: format!(
                    "VM image is not installed at {}. Restart the app to download it.",
                    path.display()
                ),
            },
            VmError::ImageDownloadFailed { reason } => Self::VmSetupFailed {
                reason: format!("vm image download failed: {reason}"),
            },
            VmError::ConfigInvalid { reason } => Self::InvalidInput {
                field: "vm_config".into(),
                reason,
            },
            VmError::Io { reason } => Self::StorageFailed { reason },
            VmError::CommandFailed { reason } | VmError::FileWatchFailed { reason } => {
                Self::Internal { reason }
            }
            VmError::SetupFailed { reason } => Self::VmSetupFailed { reason },
            VmError::NotRunning => Self::VmNotRunning,
        }
    }
}

/// Convert `AppError` to String for Tauri command return types
impl From<AppError> for String {
    fn from(e: AppError) -> Self {
        e.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_found_display() {
        let err = AppError::NotFound {
            entity: "project".into(),
            id: "abc123".into(),
        };
        assert_eq!(format!("{err}"), "cannot find project 'abc123'");
    }

    #[test]
    fn test_serialization() {
        let err = AppError::NotFound {
            entity: "project".into(),
            id: "abc".into(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"code\":\"NotFound\""));
        assert!(json.contains("\"entity\":\"project\""));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::StorageFailed { .. }));
        let reason = format!("{app_err}");
        assert!(reason.contains("file not found"));
        assert!(reason.contains("kind: NotFound"));
    }

    #[test]
    fn test_format_with_source_chain_unwraps_nested_sources() {
        #[derive(Debug)]
        struct Outer(InnerWithSource);
        #[derive(Debug)]
        struct InnerWithSource(std::io::Error);

        impl std::fmt::Display for Outer {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "outer message")
            }
        }
        impl std::error::Error for Outer {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }

        impl std::fmt::Display for InnerWithSource {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "middle hop")
            }
        }
        impl std::error::Error for InnerWithSource {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }

        let inner_io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err = Outer(InnerWithSource(inner_io));
        let formatted = format_with_source_chain("test", &err);

        assert!(formatted.starts_with("test: outer message"));
        assert!(formatted.contains("caused by: middle hop"));
        assert!(formatted.contains("caused by: denied"));
    }

    #[test]
    fn test_format_with_source_chain_no_source() {
        let err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let formatted = format_with_source_chain("ctx", &err);

        assert_eq!(formatted, "ctx: missing");
    }
}
