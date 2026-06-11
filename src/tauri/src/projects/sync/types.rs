//! Sync domain types, events, and policy
//!
//! Provides serializable state and events for project sync status, suitable for
//! frontend consumption. Uses plain-language status keys and messages (no git jargon).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::settings::AppSettings;

/// Current sync state for a project
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncState {
    /// No sync activity
    Idle,
    /// Waiting for changes to settle before saving
    Collecting,
    /// Saving changes locally
    Committing,
    /// Sending changes to remote
    Pushing,
    /// All changes saved and synced with remote
    Synced,
    /// Local changes ready to send, not yet uploaded
    PendingRemote,
    /// Remote has changes that conflict with local
    Conflict,
    /// Authentication needed to continue
    AuthRequired,
    /// Sync failed
    Error,
}

/// Serializable event payload for frontend sync status updates
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncEvent {
    /// Project identifier
    pub project_id: String,
    /// Current sync state
    pub state: SyncState,
    /// Optional human-readable detail (e.g. error message)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl SyncEvent {
    pub fn new(project_id: impl Into<String>, state: SyncState) -> Self {
        Self {
            project_id: project_id.into(),
            state,
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Policy controlling when and how sync runs
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPolicy {
    /// Whether background auto-sync is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Seconds to wait after last change before saving locally
    #[serde(default = "default_debounce_secs")]
    pub debounce_secs: u64,
    /// Seconds between automatic upload attempts
    #[serde(default = "default_push_interval_secs")]
    pub push_interval_secs: u64,
    /// Upload changes when user stops the project
    #[serde(default = "default_push_on_stop")]
    pub push_on_stop: bool,
}

impl Default for SyncPolicy {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            debounce_secs: default_debounce_secs(),
            push_interval_secs: default_push_interval_secs(),
            push_on_stop: default_push_on_stop(),
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn default_debounce_secs() -> u64 {
    45
}

fn default_push_interval_secs() -> u64 {
    300
}

fn default_push_on_stop() -> bool {
    true
}

impl From<&AppSettings> for SyncPolicy {
    fn from(settings: &AppSettings) -> Self {
        Self {
            enabled: settings.auto_sync_enabled,
            debounce_secs: u64::from(settings.sync_debounce_seconds),
            push_interval_secs: u64::from(settings.sync_push_interval_seconds),
            push_on_stop: settings.sync_push_on_stop,
        }
    }
}

impl SyncPolicy {
    pub fn debounce(&self) -> Duration {
        Duration::from_secs(self.debounce_secs)
    }

    pub fn push_interval(&self) -> Duration {
        Duration::from_secs(self.push_interval_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_state_serialization() {
        let json = serde_json::to_string(&SyncState::Synced).unwrap();
        assert_eq!(json, r#""synced""#);
        let parsed: SyncState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, SyncState::Synced);
    }

    #[test]
    fn test_sync_event_new() {
        let event = SyncEvent::new("proj-123", SyncState::Pushing);
        assert_eq!(event.project_id, "proj-123");
        assert_eq!(event.state, SyncState::Pushing);
        assert!(event.detail.is_none());
    }

    #[test]
    fn test_sync_event_with_detail() {
        let event = SyncEvent::new("proj-1", SyncState::Error).with_detail("Connection refused");
        assert_eq!(event.detail.as_deref(), Some("Connection refused"));
    }

    #[test]
    fn test_sync_event_serialization() {
        let event = SyncEvent::new("proj-456", SyncState::PendingRemote);
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("proj-456"));
        assert!(!json.contains("detail"));
        let parsed: SyncEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.project_id, event.project_id);
        assert_eq!(parsed.state, event.state);
    }

    #[test]
    fn test_sync_policy_defaults() {
        let policy = SyncPolicy::default();
        assert!(policy.enabled);
        assert_eq!(policy.debounce_secs, 45);
        assert_eq!(policy.push_interval_secs, 300);
        assert!(policy.push_on_stop);
    }

    #[test]
    fn test_sync_policy_durations() {
        let policy = SyncPolicy::default();
        assert_eq!(policy.debounce(), Duration::from_secs(45));
        assert_eq!(policy.push_interval(), Duration::from_mins(5));
    }

    #[test]
    fn test_sync_event_detail_roundtrip() {
        let event = SyncEvent::new("proj-1", SyncState::Error).with_detail("Oops");
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("detail"));
        let parsed: SyncEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.detail.as_deref(), Some("Oops"));
    }

    #[test]
    fn test_sync_policy_from_app_settings() {
        let app = AppSettings {
            vm_memory_mb: 2048,
            auto_start_vm: true,
            start_on_login: false,
            auto_sync_enabled: false,
            sync_debounce_seconds: 60,
            sync_push_interval_seconds: 900,
            sync_push_on_stop: false,
        };
        let policy = SyncPolicy::from(&app);
        assert!(!policy.enabled);
        assert_eq!(policy.debounce_secs, 60);
        assert_eq!(policy.push_interval_secs, 900);
        assert!(!policy.push_on_stop);
    }

    #[test]
    fn test_sync_policy_partial_deserialize_defaults() {
        let parsed: SyncPolicy = serde_json::from_str(r#"{"debounceSecs":30}"#).unwrap();
        assert!(parsed.enabled);
        assert_eq!(parsed.debounce_secs, 30);
        assert_eq!(parsed.push_interval_secs, 300);
        assert!(parsed.push_on_stop);
    }
}
