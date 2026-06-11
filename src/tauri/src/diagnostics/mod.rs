//! Tester-facing diagnostics: log export, version surface, redacted state.
//!
//! Exposed as Tauri commands consumed from the Settings panel and the React
//! error boundary. Everything here runs locally; nothing is auto-uploaded.

pub mod commands;
pub mod export;
