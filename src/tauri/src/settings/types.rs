//! Settings types

use serde::{Deserialize, Serialize};

/// Application settings
#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool maps to an independent user-facing toggle in the settings UI"
)]
pub struct AppSettings {
    /// VM memory allocation in MB
    #[serde(default = "default_vm_memory_mb")]
    pub vm_memory_mb: u32,

    /// Whether to auto-start VM on app launch
    #[serde(default = "default_auto_start_vm")]
    pub auto_start_vm: bool,

    /// Whether to start app on system login
    #[serde(default)]
    pub start_on_login: bool,

    // --- Sync policy settings ---
    /// Whether to auto-sync repos (pull on interval)
    #[serde(default = "default_auto_sync_enabled")]
    pub auto_sync_enabled: bool,

    /// Debounce delay in seconds before syncing after changes
    #[serde(default = "default_sync_debounce_seconds")]
    pub sync_debounce_seconds: u32,

    /// Interval in seconds between push operations
    #[serde(default = "default_sync_push_interval_seconds")]
    pub sync_push_interval_seconds: u32,

    /// Whether to push when stopping a project
    #[serde(default = "default_sync_push_on_stop")]
    pub sync_push_on_stop: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            vm_memory_mb: default_vm_memory_mb(),
            auto_start_vm: default_auto_start_vm(),
            start_on_login: false,
            auto_sync_enabled: default_auto_sync_enabled(),
            sync_debounce_seconds: default_sync_debounce_seconds(),
            sync_push_interval_seconds: default_sync_push_interval_seconds(),
            sync_push_on_stop: default_sync_push_on_stop(),
        }
    }
}

fn default_vm_memory_mb() -> u32 {
    2048
}

fn default_auto_start_vm() -> bool {
    true
}

fn default_auto_sync_enabled() -> bool {
    true
}

fn default_sync_debounce_seconds() -> u32 {
    45
}

fn default_sync_push_interval_seconds() -> u32 {
    300
}

fn default_sync_push_on_stop() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = AppSettings::default();
        assert_eq!(settings.vm_memory_mb, 2048);
        assert!(settings.auto_start_vm);
        assert!(!settings.start_on_login);
        // Sync policy defaults
        assert!(settings.auto_sync_enabled);
        assert_eq!(settings.sync_debounce_seconds, 45);
        assert_eq!(settings.sync_push_interval_seconds, 300);
        assert!(settings.sync_push_on_stop);
    }

    #[test]
    fn test_serde_roundtrip() {
        let settings = AppSettings {
            vm_memory_mb: 4096,
            auto_start_vm: false,
            start_on_login: true,
            auto_sync_enabled: false,
            sync_debounce_seconds: 60,
            sync_push_interval_seconds: 600,
            sync_push_on_stop: false,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.vm_memory_mb, 4096);
        assert!(!parsed.auto_start_vm);
        assert!(parsed.start_on_login);
        assert!(!parsed.auto_sync_enabled);
        assert_eq!(parsed.sync_debounce_seconds, 60);
        assert_eq!(parsed.sync_push_interval_seconds, 600);
        assert!(!parsed.sync_push_on_stop);
    }

    #[test]
    fn test_backward_compatibility_old_settings_file() {
        // Old state.json without sync policy fields should deserialize with defaults
        let legacy_json = r#"{"vmMemoryMb":2048,"autoStartVm":true,"startOnLogin":false}"#;
        let parsed: AppSettings = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(parsed.vm_memory_mb, 2048);
        assert!(parsed.auto_start_vm);
        assert!(!parsed.start_on_login);
        // New fields get defaults
        assert!(parsed.auto_sync_enabled);
        assert_eq!(parsed.sync_debounce_seconds, 45);
        assert_eq!(parsed.sync_push_interval_seconds, 300);
        assert!(parsed.sync_push_on_stop);
    }

    #[test]
    fn test_partial_sync_fields_use_defaults_for_missing() {
        // Only some sync fields present; missing ones get defaults
        let partial_json = r#"{"vmMemoryMb":1024,"autoSyncEnabled":false}"#;
        let parsed: AppSettings = serde_json::from_str(partial_json).unwrap();
        assert_eq!(parsed.vm_memory_mb, 1024);
        assert!(!parsed.auto_sync_enabled);
        assert_eq!(parsed.sync_debounce_seconds, 45);
        assert_eq!(parsed.sync_push_interval_seconds, 300);
        assert!(parsed.sync_push_on_stop);
    }
}
