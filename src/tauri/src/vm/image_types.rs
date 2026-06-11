//! Cross-platform serializable types for the VM image downloader.
//!
//! The downloader itself is macOS-only (lives in `image_downloader.rs`,
//! gated by `#![cfg(target_os = "macos")]`). The types below are not
//! gated so platform stubs on Linux/Windows can return them without the
//! specta builder needing two definitions.

use serde::Serialize;

/// Whether the local image is current, plus size hints for the welcome screen.
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VmImageStatus {
    pub needed: bool,
    pub installed_version: Option<String>,
    pub target_version: Option<String>,
    pub compressed_size_bytes: Option<u64>,
    pub uncompressed_size_bytes: Option<u64>,
}

/// Emitted on the `vm-image-progress` Tauri event. Byte counts are over the
/// compressed payload (matches what the user sees as MB/s).
#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "phase")]
pub enum VmImageProgress {
    Started { total: u64 },
    Downloading { downloaded: u64, total: u64 },
    Verifying,
    Decompressing,
    Finished,
}
