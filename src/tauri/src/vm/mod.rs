//! VM module for running Linux containers on non-Linux platforms
//!
//! On macOS, we need a Linux VM to run containers:
//! - Apple Virtualization.framework via vfkit on ARM and Intel Macs
//!
//! On Windows, we use WSL2 to run containers:
//! - WSL2: Windows Subsystem for Linux 2 (preferred, excellent performance)
//! - QEMU: Fallback when WSL2 is unavailable

pub mod clone;
pub mod error;
pub mod image_types;

#[cfg(any(
    all(target_os = "macos", target_arch = "x86_64"),
    target_os = "windows"
))]
pub mod shell;

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub mod qemu;

#[cfg(target_os = "macos")]
pub mod image_downloader;

#[cfg(target_os = "macos")]
pub mod vfkit;

#[cfg(target_os = "macos")]
pub mod agent_client;

#[cfg(target_os = "macos")]
pub mod file_watcher;

#[cfg(target_os = "macos")]
pub mod gateway;

#[cfg(target_os = "macos")]
pub mod lifecycle;

#[cfg(target_os = "macos")]
pub mod port_forward;

#[cfg(target_os = "macos")]
pub mod runtime;

#[cfg(target_os = "macos")]
pub use gateway::{ContainerStatus, ResourceStats, VmGateway};

#[cfg(target_os = "macos")]
pub use runtime::VmRuntime;

#[cfg(target_os = "macos")]
pub use lifecycle::{VmLifecycle, VmStatus};

#[cfg(target_os = "macos")]
pub use vfkit::VfkitVM;

#[cfg(target_os = "macos")]
pub use image_downloader::VmImageDownloader;

pub use image_types::{VmImageProgress, VmImageStatus};

#[cfg(target_os = "windows")]
pub mod wsl;

#[cfg(target_os = "windows")]
pub mod wsl_setup;

#[cfg(target_os = "windows")]
pub mod qemu_windows;

mod traits;
pub use self::error::VmError;
pub use traits::VirtualMachine;
