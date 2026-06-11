//! Domain errors for the in-VM agent.
//!
//! Wraps third-party transport errors (`bollard`, `std::io`) behind
//! per-operation variants so the public API does not leak external types
//! and so each failure carries the operation context the caller needs.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("cannot connect to podman socket {socket}: {source}")]
    PodmanConnect {
        socket: PathBuf,
        #[source]
        source: bollard::errors::Error,
    },
    #[error("cannot start container '{name}': {source}")]
    ContainerStart {
        name: String,
        #[source]
        source: bollard::errors::Error,
    },
    #[error("cannot stop container '{id}': {source}")]
    ContainerStop {
        id: String,
        #[source]
        source: bollard::errors::Error,
    },
    #[error("cannot remove container '{id}': {source}")]
    ContainerRemove {
        id: String,
        #[source]
        source: bollard::errors::Error,
    },
    #[error("cannot inspect container '{id}': {source}")]
    ContainerInspect {
        id: String,
        #[source]
        source: bollard::errors::Error,
    },
    #[error("cannot stream logs from '{id}': {source}")]
    LogStream {
        id: String,
        #[source]
        source: bollard::errors::Error,
    },
    #[error("cannot list containers: {source}")]
    ContainerList {
        #[source]
        source: bollard::errors::Error,
    },
    #[error("cannot bind vsock listener (cid={cid} port={port}): {source}")]
    VsockBind {
        cid: u32,
        port: u32,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot {op} {path}: {source} (kind: {kind:?})")]
    Io {
        op: String,
        path: PathBuf,
        kind: std::io::ErrorKind,
        #[source]
        source: std::io::Error,
    },
    #[error("{message}")]
    Other { message: String },
}

impl AgentError {
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other {
            message: message.into(),
        }
    }
}

// Compatibility: agent ttrpc handlers historically returned String. Until
// they're fully migrated, this conversion lets `?` keep working with String
// error sites and lets `internal_err(e.into())` accept `AgentError` directly.
impl From<AgentError> for String {
    fn from(e: AgentError) -> Self {
        e.to_string()
    }
}
