//! Domain errors for VM lifecycle operations.
//!
//! Converted to `AppError` at Tauri command boundaries via `From<VmError>`.

use std::path::PathBuf;

use serde::Serialize;
use thiserror::Error;

/// Structured errors for VM operations (launch, shutdown, connection, setup, io).
#[derive(Clone, Debug, Error, Serialize)]
pub enum VmError {
    #[error("cannot launch vm: {reason}")]
    LaunchFailed { reason: String },

    #[error("cannot stop vm: {reason}")]
    ShutdownFailed { reason: String },

    #[error("cannot reach vm: {reason}")]
    ConnectionFailed { reason: String },

    #[error("cannot find vm image at {path}")]
    ImageNotFound { path: PathBuf },

    #[error("cannot download vm image: {reason}")]
    ImageDownloadFailed { reason: String },

    #[error("cannot validate vm configuration: {reason}")]
    ConfigInvalid { reason: String },

    #[error("cannot complete io operation: {reason}")]
    Io { reason: String },

    #[error("cannot execute command: {reason}")]
    CommandFailed { reason: String },

    #[error("cannot complete setup: {reason}")]
    SetupFailed { reason: String },

    #[error("cannot reach vm: not running")]
    NotRunning,

    #[error("cannot watch files: {reason}")]
    FileWatchFailed { reason: String },
}

impl From<std::io::Error> for VmError {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            reason: format!("{err} (kind: {:?})", err.kind()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::VmError;

    #[test]
    fn from_io_error_preserves_kind() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let vm_err: VmError = io_err.into();
        assert!(matches!(vm_err, VmError::Io { .. }));
        let reason = format!("{vm_err}");
        assert!(reason.contains("denied"));
        assert!(reason.contains("kind: PermissionDenied"));
    }
}
