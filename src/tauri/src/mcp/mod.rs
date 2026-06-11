//! MCP listener feature module.
//!
//! Exposes Opnble's project / VM / publish surface to coding agents over
//! Model Context Protocol. The listener owns the bearer-token auth, the
//! slug-to-project resolver, and the `AppError` -> `rmcp::ErrorData`
//! mapper; the tool implementations live in `tools/` and call into the
//! existing `ProjectStore`, `ProjectOrchestrator`, `GitOps`, `Runtime`,
//! `Authenticator`, `TunnelCoordinator`, and `GitHubPagesClient` so MCP
//! and the Tauri UI stay one client of one core (no business-logic
//! duplication).

pub mod auth;
pub mod commands;
pub mod config;
pub mod errors;
pub(crate) mod exec;
pub mod install;
pub(crate) mod orch;
pub(crate) mod path;
pub mod resolve;
pub mod ring;
pub mod server;
pub mod wait;

mod tools;

pub use server::{bind_listener, spawn, McpDeps, McpHandle};
