//! GitHub Pages publishing
//!
//! Detects the project framework, runs a static build in a temporary
//! container, pushes the output to an orphan `gh-pages` branch, and
//! enables GitHub Pages via the REST API.

pub mod builder;
pub mod commands;
pub mod deployer;
pub mod error;
pub mod github_pages;
pub mod types;

pub use error::PublishError;
