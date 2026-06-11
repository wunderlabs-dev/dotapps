//! Authentication feature module
//!
//! Provides authentication services for Git providers including:
//! - Secure token storage via OS keychain
//! - GitHub Device OAuth flow
//! - GitHub API integration (user info, repositories)
//!
//! # Architecture
//!
//! ```text
//! commands.rs (Tauri handlers)
//!      │
//!      ▼
//! authenticator.rs (Business logic)
//!      │
//!      ▼
//! store.rs (Token storage abstraction)
//!      │
//!      ▼
//! infrastructure::Keychain (OS keychain)
//! ```

mod authenticator;
pub mod commands;
pub mod github_repos;
mod store;
mod types;

pub use self::authenticator::Authenticator;
#[allow(
    unused_imports,
    reason = "CreatedRepo is part of the github_repos public API; future MCP tools will consume it"
)]
pub use self::github_repos::{CreatedRepo, GitHubReposClient};
pub use self::store::KeychainTokenStore;
