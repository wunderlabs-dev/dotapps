//! Tool implementations exposed by the MCP server.
//!
//! Phase 2 lands the discovery and lifecycle tools (`list_projects`,
//! `get_project`, `start_project`, `stop_project`, `restart_project`).
//! All five tools live in one `#[tool_router]` impl block in `discovery.rs`
//! because rmcp emits a single `tool_router()` fn per impl, and splitting
//! them would force per-file routers and extra macro plumbing.
//!
//! `types` holds the agent-facing view types (`ProjectView`, `RunningView`,
//! `StatusView`) the tool bodies return as `Json<T>` structured content.
//! Phases 3-7 add `tail_logs`, git tools, share/unshare, publish, and VM
//! tools to the same impl block.

pub mod discovery;
pub mod types;
