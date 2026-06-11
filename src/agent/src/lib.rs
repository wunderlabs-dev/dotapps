//! In-VM ttrpc agent that the Opnble host drives over vsock.
//!
//! Reach for this crate for guest-VM work: container lifecycle, health
//! probes, the ttrpc service, and the vsock port-forward mux.

use std::path::Path;

pub mod container;
pub mod error;
#[allow(
    clippy::all,
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    reason = "auto-generated protobuf/ttrpc code"
)]
pub mod generated;
pub mod health;
pub mod port_forward;
pub mod service;

pub use error::AgentError;

/// Format an `io::Error` with path, operation, and `ErrorKind` for the
/// agent's `String`-typed error paths in `service.rs`. `AgentError`
/// carries this context structurally for sites it covers.
pub(crate) fn io_error_context(op: &str, path: &Path, e: &std::io::Error) -> String {
    format!("cannot {op} {}: {e} (kind: {:?})", path.display(), e.kind())
}
