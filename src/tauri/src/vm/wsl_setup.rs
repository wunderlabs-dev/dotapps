//! WSL2 setup and enablement for Windows

use std::process::Command;

use super::VmError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WSLStatus {
    Ready,               // WSL2 is ready to use
    NeedsEnable,         // WSL feature not enabled
    NeedsVirtualization, // Virtualization not enabled in BIOS
    NeedsReboot,         // Reboot required after enabling
    NotSupported,        // Windows version too old
    Unknown(String),
}

impl std::fmt::Display for WSLStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ready => write!(f, "ready"),
            Self::NeedsEnable => write!(f, "needs_enable"),
            Self::NeedsVirtualization => write!(f, "needs_virtualization"),
            Self::NeedsReboot => write!(f, "needs_reboot"),
            Self::NotSupported => write!(f, "not_supported"),
            Self::Unknown(msg) => write!(f, "unknown:{msg}"),
        }
    }
}

/// Check the current status of WSL2
pub fn check_wsl_status() -> WSLStatus {
    // Check Windows version
    if !is_windows_version_supported() {
        return WSLStatus::NotSupported;
    }

    // Check if WSL is installed
    let wsl_check = Command::new("wsl").args(["--status"]).output();

    match wsl_check {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("Default Version: 2") || stdout.contains("WSL 2") {
                WSLStatus::Ready
            } else {
                WSLStatus::NeedsEnable
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not recognized") {
                WSLStatus::NeedsEnable
            } else if stderr.contains("virtualization") {
                WSLStatus::NeedsVirtualization
            } else {
                WSLStatus::Unknown(stderr.to_string())
            }
        }
        Err(_) => WSLStatus::NeedsEnable,
    }
}

/// Check if Windows version supports WSL2
fn is_windows_version_supported() -> bool {
    // Use winreg on Windows, stub for other platforms
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(key) = hklm.open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion") {
            if let Ok(build) = key.get_value::<String, _>("CurrentBuild") {
                // WSL2 requires Windows 10 build 19041 or higher
                if let Ok(build_num) = build.parse::<u32>() {
                    return build_num >= 19041;
                }
            }
        }
        false
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On non-Windows, always return false (not supported)
        false
    }
}

/// Enable WSL2 (requires admin privileges)
pub fn enable_wsl() -> Result<bool, VmError> {
    // This requires running as admin
    // We'll create a PowerShell script and run it elevated

    let script = r#"
        # Enable WSL feature
        dism.exe /online /enable-feature /featurename:Microsoft-Windows-Subsystem-Linux /all /norestart

        # Enable Virtual Machine Platform
        dism.exe /online /enable-feature /featurename:VirtualMachinePlatform /all /norestart

        # Set WSL2 as default
        wsl --set-default-version 2
    "#;

    // Save script to temp file
    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("opnble-enable-wsl.ps1");
    std::fs::write(&script_path, script)?;

    // Run as admin
    let output = Command::new("powershell")
        .args([
            "-Command",
            &format!(
                "Start-Process powershell -ArgumentList '-ExecutionPolicy Bypass -File \"{}\"' -Verb RunAs -Wait",
                script_path.display()
            ),
        ])
        .output()
        .map_err(|e| VmError::SetupFailed {
            reason: format!("cannot run wsl setup: {e}"),
        })?;

    // Check if reboot is needed
    if output.status.success() {
        // Check if reboot is pending using registry
        #[cfg(target_os = "windows")]
        {
            use winreg::enums::*;
            use winreg::RegKey;

            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
            if hklm
                .open_subkey(
                    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Component Based Servicing\\RebootPending",
                )
                .is_ok()
            {
                return Ok(true); // Reboot needed
            }
            if hklm
                .open_subkey(
                    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\WindowsUpdate\\Auto Update\\RebootRequired",
                )
                .is_ok()
            {
                return Ok(true); // Reboot needed
            }
        }
        Ok(false) // No reboot needed
    } else {
        Err(VmError::SetupFailed {
            reason: format!(
                "cannot enable wsl: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        })
    }
}
