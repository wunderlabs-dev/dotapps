//! Project types

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::constants::{containers, events, paths};
use crate::error::AppError;

/// Error type for invalid project IDs
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectIdError {
    Empty,
    InvalidCharacters,
}

impl fmt::Display for ProjectIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "cannot use empty project id"),
            Self::InvalidCharacters => {
                write!(
                    f,
                    "cannot use project id with invalid characters (only alphanumeric and dash allowed)"
                )
            }
        }
    }
}

impl std::error::Error for ProjectIdError {}

/// A validated project identifier
#[derive(Clone, Debug, Eq, Hash, PartialEq, specta::Type)]
#[specta(transparent)]
pub struct ProjectId(String);

impl ProjectId {
    /// Create a new `ProjectId`, validating the input
    pub fn new(id: impl Into<String>) -> Result<Self, ProjectIdError> {
        let id = id.into();

        if id.is_empty() {
            return Err(ProjectIdError::Empty);
        }

        if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(ProjectIdError::InvalidCharacters);
        }

        Ok(Self(id))
    }

    /// Generate a new unique project ID
    pub fn generate() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_millis();
        // This is safe because we're only using alphanumeric chars
        Self(format!("proj-{timestamp}"))
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[inline]
    pub fn container_name(&self) -> String {
        containers::name(&self.0)
    }

    #[inline]
    pub fn log_event(&self) -> String {
        events::project_log(&self.0)
    }

    #[inline]
    pub fn repo_path(&self) -> Result<PathBuf, AppError> {
        paths::project_repo(&self.0)
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ProjectId> for String {
    fn from(id: ProjectId) -> Self {
        id.0
    }
}

impl TryFrom<String> for ProjectId {
    type Error = ProjectIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ProjectId::new(value)
    }
}

impl AsRef<str> for ProjectId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Serialize for ProjectId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProjectId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ProjectId::new(s).map_err(serde::de::Error::custom)
    }
}

/// Whether the user wants this project running or stopped
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProjectIntent {
    Run,
    #[default]
    Stop,
}

/// Project lifecycle status (derived by the orchestrator from observed state)
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ProjectStatus {
    #[default]
    Stopped,
    WaitingForVm,
    Starting,
    Running,
    Ready,
    Degraded,
    Failed,
    Fixing,
    FixFailed,
    Stopping,
}

impl ProjectStatus {
    /// Whether the project is in a state where periodic sync should keep running.
    ///
    /// Returns true for states where the container is active and user file
    /// changes may occur: Running (installing), Ready (serving), Degraded (unhealthy).
    pub fn is_syncable(&self) -> bool {
        matches!(self, Self::Running | Self::Ready | Self::Degraded)
    }
}

impl fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped => write!(f, "stopped"),
            Self::WaitingForVm => write!(f, "waitingForVm"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Ready => write!(f, "ready"),
            Self::Degraded => write!(f, "degraded"),
            Self::Failed => write!(f, "failed"),
            Self::Fixing => write!(f, "fixing"),
            Self::FixFailed => write!(f, "fixFailed"),
            Self::Stopping => write!(f, "stopping"),
        }
    }
}

/// Steps in the first-run install pipeline.
#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum InstallStep {
    VmStarting,
    AllocatingResources,
    NpmInstalling,
    RunningServer,
}

impl fmt::Display for InstallStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VmStarting => write!(f, "vmStarting"),
            Self::AllocatingResources => write!(f, "allocatingResources"),
            Self::NpmInstalling => write!(f, "npmInstalling"),
            Self::RunningServer => write!(f, "runningServer"),
        }
    }
}

/// Status of an individual install step.
#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum InstallStepStatus {
    Pending,
    Active,
    Done,
    Failed { reason: String },
}

/// Event payload emitted on `project-install-step-{id}`.
#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InstallStepEvent {
    pub step: InstallStep,
    pub status: InstallStepStatus,
}

/// A project in the application
#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    /// Portable, kebab-case identity derived from the repo URL.
    ///
    /// Survives `git clone` to a different machine, while `id` is local-only
    /// (`proj-{unix_millis}` from the importing machine). Persisted in
    /// `~/.opnble/state.json` and committed into the repo's future
    /// `.opnble` marker so MCP tools can resolve a slug back to a local
    /// project on any machine that has imported the repo.
    #[serde(default)]
    pub slug: Option<String>,
    pub repo_url: String,
    pub local_path: String,
    pub status: ProjectStatus,
    #[serde(default)]
    pub intent: ProjectIntent,
    pub port: Option<u16>,
    pub branch: String,
    pub last_synced_at: u64,
    #[serde(default)]
    pub tunnel_id: Option<String>,
    #[serde(default)]
    pub tunnel_url: Option<String>,
    #[serde(default)]
    pub pages_url: Option<String>,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub parent_project_id: Option<ProjectId>,
    #[serde(default)]
    pub forked_at: Option<u64>,
}

impl Project {
    /// Create a new project
    pub fn new(
        id: ProjectId,
        name: String,
        repo_url: String,
        local_path: String,
        branch: String,
    ) -> Self {
        Self {
            id,
            name,
            slug: None,
            repo_url,
            local_path,
            status: ProjectStatus::Stopped,
            intent: ProjectIntent::Stop,
            port: None,
            branch,
            last_synced_at: unix_now_millis(),
            tunnel_id: None,
            tunnel_url: None,
            pages_url: None,
            installed: false,
            parent_project_id: None,
            forked_at: None,
        }
    }

    /// Use during import to attach a portable, collision-free slug to the
    /// project. Pair with [`crate::projects::slug::derive_slug_unique`] to
    /// compute the slug; the project should already be in (or about to
    /// enter) the store so the uniqueness check sees it.
    #[must_use]
    pub fn with_slug(mut self, slug: String) -> Self {
        self.slug = Some(slug);
        self
    }

    /// Mark this project as a fork of `parent`, captured at `forked_at`
    /// (Unix millis). Pair with snapshot rollback semantics so the UI can
    /// surface parent lineage and discard forks without affecting the
    /// upstream project.
    #[must_use]
    pub fn with_parent(mut self, parent: ProjectId, forked_at: u64) -> Self {
        self.parent_project_id = Some(parent);
        self.forked_at = Some(forked_at);
        self
    }

    /// Use when reading or writing files in the cloned repo on disk.
    ///
    /// Returns the absolute path to the project's local checkout.
    pub fn repo_path(&self) -> PathBuf {
        PathBuf::from(&self.local_path)
    }
}

/// Get current Unix timestamp in milliseconds (for JavaScript `Date` / timeago.js)
pub fn unix_now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_millis()
        .try_into()
        .expect("timestamp millis exceeds u64 (not possible before year 584,942,417)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_id_valid() {
        assert!(ProjectId::new("abc123").is_ok());
        assert!(ProjectId::new("my-project").is_ok());
    }

    #[test]
    fn test_project_id_invalid() {
        assert!(ProjectId::new("").is_err());
        assert!(ProjectId::new("my_project").is_err());
        assert!(ProjectId::new("my project").is_err());
    }

    #[test]
    fn test_project_id_generate() {
        let id = ProjectId::generate();
        assert!(id.as_str().starts_with("proj-"));
    }

    #[test]
    fn test_project_tunnel_fields_default_to_none() {
        let project = Project::new(
            ProjectId::new("test-proj").unwrap(),
            "Test".to_string(),
            "https://example.com/repo".to_string(),
            "/tmp/test".to_string(),
            "main".to_string(),
        );
        assert!(project.tunnel_id.is_none());
        assert!(project.tunnel_url.is_none());
        assert!(project.slug.is_none());
    }

    #[test]
    fn project_with_slug_attaches_slug() {
        let project = Project::new(
            ProjectId::new("test-proj").unwrap(),
            "Test".to_string(),
            "https://example.com/repo".to_string(),
            "/tmp/test".to_string(),
            "main".to_string(),
        )
        .with_slug("my-blog".to_string());
        assert_eq!(project.slug.as_deref(), Some("my-blog"));
    }

    #[test]
    fn project_deserialize_without_slug_field() {
        let json = r#"{
            "id": "test-proj",
            "name": "Test",
            "repoUrl": "https://example.com/repo",
            "localPath": "/tmp/test",
            "status": "stopped",
            "intent": "stop",
            "port": null,
            "branch": "main",
            "lastSyncedAt": 0
        }"#;
        let project: Project = serde_json::from_str(json).unwrap();
        assert!(project.slug.is_none());
    }

    #[test]
    fn test_project_deserialize_without_tunnel_fields() {
        let json = r#"{
            "id": "test-proj",
            "name": "Test",
            "repoUrl": "https://example.com/repo",
            "localPath": "/tmp/test",
            "status": "stopped",
            "intent": "stop",
            "port": null,
            "branch": "main",
            "lastSyncedAt": 0
        }"#;
        let project: Project = serde_json::from_str(json).unwrap();
        assert!(project.tunnel_id.is_none());
        assert!(project.tunnel_url.is_none());
    }

    #[test]
    fn project_deserialize_without_parent_fields() {
        let json = r#"{
            "id": "test-proj", "name": "Test",
            "repoUrl": "https://example.com/repo", "localPath": "/tmp/test",
            "status": "stopped", "intent": "stop", "port": null,
            "branch": "main", "lastSyncedAt": 0
        }"#;
        let project: Project = serde_json::from_str(json).unwrap();
        assert!(project.parent_project_id.is_none());
        assert!(project.forked_at.is_none());
    }

    #[test]
    fn project_with_parent_id_carries_parent_and_forked_at() {
        let parent = ProjectId::new("proj-parent").unwrap();
        let project = Project::new(
            ProjectId::new("proj-child").unwrap(),
            "Child".to_string(),
            "https://example.com/repo".to_string(),
            "/tmp/child".to_string(),
            "main".to_string(),
        )
        .with_parent(parent.clone(), 1_234_567_890);
        assert_eq!(project.parent_project_id.as_ref(), Some(&parent));
        assert_eq!(project.forked_at, Some(1_234_567_890));
    }

    #[test]
    fn syncable_statuses() {
        let syncable = [
            ProjectStatus::Running,
            ProjectStatus::Ready,
            ProjectStatus::Degraded,
        ];
        for status in &syncable {
            assert!(status.is_syncable(), "{status} should be syncable");
        }

        let not_syncable = [
            ProjectStatus::Stopped,
            ProjectStatus::WaitingForVm,
            ProjectStatus::Starting,
            ProjectStatus::Failed,
            ProjectStatus::Fixing,
            ProjectStatus::FixFailed,
            ProjectStatus::Stopping,
        ];
        for status in &not_syncable {
            assert!(!status.is_syncable(), "{status} should not be syncable");
        }
    }
}
