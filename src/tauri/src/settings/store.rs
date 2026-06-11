//! Settings storage
#![expect(
    clippy::disallowed_types,
    reason = "sync file I/O locking, not used across await points"
)]

use std::path::PathBuf;
use std::sync::Mutex;

use crate::constants::{paths, ports};
use crate::error::AppError;
use crate::infrastructure::{AtomicJsonFile, LoadOutcome, RecoveryReport};
use crate::settings::types::AppSettings;

fn default_next_port() -> u16 {
    ports::DEFAULT_NEXT_PORT
}

/// Internal state structure for JSON storage
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredState {
    #[serde(default)]
    settings: AppSettings,
    // Other fields from state.json that we need to preserve
    #[serde(default)]
    projects: Vec<serde_json::Value>,
    #[serde(default = "default_next_port")]
    next_port: u16,
}

/// Trait for settings storage operations
pub trait SettingsStore: Send + Sync {
    /// Get current settings
    fn get(&self) -> Result<AppSettings, AppError>;

    /// Save settings
    fn save(&self, settings: AppSettings) -> Result<(), AppError>;
}

/// JSON file-based settings store
pub struct JsonSettingsStore {
    state: Mutex<StoredState>,
    file: AtomicJsonFile,
}

impl JsonSettingsStore {
    /// Load settings from disk or create default
    pub fn load_or_default() -> Result<(Self, Option<RecoveryReport>), AppError> {
        let path = paths::state_file()?;
        Ok(Self::load_from_path(path))
    }

    /// Load from a specific path (for testing)
    pub fn load_from_path(path: PathBuf) -> (Self, Option<RecoveryReport>) {
        let file = AtomicJsonFile::new(path);
        let (state, report) = match file.load::<StoredState>() {
            LoadOutcome::Fresh(s) | LoadOutcome::Loaded(s) => (s, None),
            LoadOutcome::RecoveredFromBackup(s) => (s, Some(RecoveryReport::RecoveredFromBackup)),
            LoadOutcome::ArchivedAndReset { archive_paths } => (
                StoredState::default(),
                Some(RecoveryReport::ArchivedAndReset { archive_paths }),
            ),
        };
        (
            Self {
                state: Mutex::new(state),
                file,
            },
            report,
        )
    }

    /// Save to disk, merging with latest on-disk state to avoid stomping projects/nextPort
    fn save_to_disk(&self, settings: &AppSettings) -> Result<(), AppError> {
        let mut obj: serde_json::Map<String, serde_json::Value> =
            if let serde_json::Value::Object(map) = self.file.read_raw()? {
                map
            } else {
                serde_json::Map::new()
            };
        obj.insert("settings".to_string(), serde_json::to_value(settings)?);
        self.file.save(&serde_json::Value::Object(obj))
    }
}

impl SettingsStore for JsonSettingsStore {
    fn get(&self) -> Result<AppSettings, AppError> {
        let state = self.state.lock()?;
        Ok(state.settings.clone())
    }

    fn save(&self, settings: AppSettings) -> Result<(), AppError> {
        {
            let mut state = self.state.lock()?;
            state.settings = settings.clone();
        }
        self.save_to_disk(&settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_store_trait_is_object_safe() {
        fn _accepts_dyn(_: &dyn SettingsStore) {}
    }

    #[test]
    fn test_stored_state_backward_compatibility() {
        // Old state.json format without sync policy fields in settings
        let legacy_state = r#"{
            "settings": {"vmMemoryMb":2048,"autoStartVm":true,"startOnLogin":false},
            "projects": [],
            "nextPort": 3001
        }"#;
        let state: StoredState = serde_json::from_str(legacy_state).unwrap();
        assert_eq!(state.settings.vm_memory_mb, 2048);
        assert!(state.settings.auto_sync_enabled);
        assert_eq!(state.settings.sync_debounce_seconds, 45);
        assert_eq!(state.settings.sync_push_interval_seconds, 300);
        assert!(state.settings.sync_push_on_stop);
        assert_eq!(state.next_port, 3001);
    }

    #[test]
    fn test_stored_state_backward_compatibility_missing_next_port() {
        let legacy_state = r#"{"settings":{"vmMemoryMb":2048},"projects":[]}"#;
        let state: StoredState = serde_json::from_str(legacy_state).unwrap();
        assert_eq!(state.next_port, crate::constants::ports::DEFAULT_NEXT_PORT);
    }

    #[test]
    fn test_settings_save_preserves_projects_and_next_port() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let initial: serde_json::Value = serde_json::json!({
            "settings": {"vmMemoryMb": 1024},
            "projects": [{"id":"proj-1","name":"Test","repoUrl":"https://x.com/y","localPath":"/a","status":"stopped","port":null,"branch":"main","lastSyncedAt":0}],
            "nextPort": 3005
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();
        let (store, _report) = JsonSettingsStore::load_from_path(path.clone());
        store
            .save(AppSettings {
                vm_memory_mb: 2048,
                auto_start_vm: false,
                start_on_login: false,
                auto_sync_enabled: true,
                sync_debounce_seconds: 45,
                sync_push_interval_seconds: 300,
                sync_push_on_stop: true,
            })
            .unwrap();
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let obj = content.as_object().unwrap();
        assert_eq!(
            obj.get("settings")
                .unwrap()
                .as_object()
                .unwrap()
                .get("vmMemoryMb")
                .unwrap()
                .as_u64()
                .unwrap(),
            2048
        );
        assert_eq!(obj.get("projects").unwrap().as_array().unwrap().len(), 1);
        assert_eq!(obj.get("nextPort").unwrap().as_u64().unwrap(), 3005);
    }

    #[test]
    fn test_stored_state_roundtrip_with_sync_settings() {
        let state = StoredState {
            settings: AppSettings {
                vm_memory_mb: 4096,
                auto_start_vm: false,
                start_on_login: true,
                auto_sync_enabled: false,
                sync_debounce_seconds: 60,
                sync_push_interval_seconds: 600,
                sync_push_on_stop: false,
            },
            projects: Vec::new(),
            next_port: 3002,
        };
        let json = serde_json::to_string(&state).unwrap();
        let parsed: StoredState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.settings.sync_debounce_seconds, 60);
        assert_eq!(parsed.settings.sync_push_interval_seconds, 600);
        assert!(!parsed.settings.sync_push_on_stop);
    }
}
