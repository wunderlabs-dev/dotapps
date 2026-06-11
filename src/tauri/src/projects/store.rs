//! Project storage
#![expect(
    clippy::disallowed_types,
    reason = "sync file I/O locking, not used across await points"
)]

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::constants::{paths, ports};
use crate::error::AppError;
use crate::infrastructure::{AtomicJsonFile, LoadOutcome, RecoveryReport};
use crate::projects::types::{Project, ProjectId};

/// Internal state structure for JSON storage
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredState {
    #[serde(default)]
    projects: Vec<Project>,
    #[serde(default = "default_next_port")]
    next_port: u16,
    #[serde(default)]
    snapshots: Vec<crate::snapshots::SnapshotRecord>,
    // Preserve settings when saving
    #[serde(default)]
    settings: serde_json::Value,
}

fn default_next_port() -> u16 {
    ports::DEFAULT_NEXT_PORT
}

impl Default for StoredState {
    fn default() -> Self {
        Self {
            projects: Vec::new(),
            next_port: ports::DEFAULT_NEXT_PORT,
            snapshots: Vec::new(),
            settings: serde_json::Value::Object(serde_json::Map::new()),
        }
    }
}

/// Trait for project storage operations
pub trait ProjectStore: Send + Sync {
    /// List all projects
    fn list(&self) -> Result<Vec<Project>, AppError>;

    /// Get a project by ID
    fn get(&self, id: &ProjectId) -> Result<Project, AppError>;

    /// Get a project by its portable slug.
    ///
    /// Default implementation does a linear scan over [`Self::list`] and
    /// skips projects whose `slug` is `None`. Implementations backed by an
    /// index can override.
    fn get_by_slug(&self, slug: &str) -> Result<Project, AppError> {
        self.list()?
            .into_iter()
            .find(|p| p.slug.as_deref() == Some(slug))
            .ok_or_else(|| AppError::NotFound {
                entity: "project".into(),
                id: slug.into(),
            })
    }

    /// Add a new project
    fn add(&self, project: Project) -> Result<(), AppError>;

    /// Update an existing project
    fn update(&self, project: Project) -> Result<(), AppError>;

    /// Remove a project by ID
    fn remove(&self, id: &ProjectId) -> Result<(), AppError>;

    /// Allocate the next available port
    fn allocate_port(&self) -> Result<u16, AppError>;

    /// Get the next port that will be allocated
    fn next_port(&self) -> Result<u16, AppError>;

    /// Persist a snapshot record. Per-project FIFO cap of 20 — adding a 21st
    /// snapshot for the same project evicts the oldest (by insertion order).
    fn add_snapshot(&self, record: crate::snapshots::SnapshotRecord) -> Result<(), AppError>;

    /// List snapshots for a project in insertion order (oldest first).
    #[allow(
        dead_code,
        reason = "consumed in Task 5.4/7.1 (rollback tool and Tauri commands)"
    )]
    fn list_snapshots(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<crate::snapshots::SnapshotRecord>, AppError>;

    /// Remove a snapshot by id. Returns `NotFound` if the snapshot is missing.
    #[allow(
        dead_code,
        reason = "consumed in Task 5.2/5.4/7.1 (snapshot tools and Tauri commands)"
    )]
    fn remove_snapshot(&self, id: &crate::snapshots::SnapshotId) -> Result<(), AppError>;

    /// Get a snapshot by id. Returns `NotFound` if the snapshot is missing.
    fn get_snapshot(
        &self,
        id: &crate::snapshots::SnapshotId,
    ) -> Result<crate::snapshots::SnapshotRecord, AppError>;
}

/// JSON file-based project store
pub struct JsonProjectStore {
    state: Mutex<StoredState>,
    file: AtomicJsonFile,
}

impl JsonProjectStore {
    /// Load projects from disk or create default
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
        let store = Self {
            state: Mutex::new(state),
            file,
        };
        if let Err(e) = store.backfill_missing_slugs() {
            tracing::warn!("cannot backfill project slugs: {e}");
        }
        (store, report)
    }

    /// Assign a portable slug to every stored project that lacks one, then
    /// persist if anything changed.
    ///
    /// Projects imported before slugs existed (or through a path that never
    /// wrote one) are unreachable by MCP tools, which resolve only by slug.
    /// Running this on load repairs that gap in place. Idempotent: a second
    /// run finds every project already slugged and writes nothing.
    fn backfill_missing_slugs(&self) -> Result<(), AppError> {
        let mut state = self.state.lock()?;
        let mut taken: HashSet<String> = state
            .projects
            .iter()
            .filter_map(|p| p.slug.clone())
            .collect();
        let mut changed = false;
        for project in &mut state.projects {
            if project.slug.is_some() {
                continue;
            }
            let slug =
                crate::projects::slug::derive_slug_unique_excluding(&taken, &project.repo_url)?;
            taken.insert(slug.clone());
            project.slug = Some(slug);
            changed = true;
        }
        if changed {
            self.save_to_disk(&state)?;
        }
        Ok(())
    }

    /// Save to disk, merging with latest on-disk state to avoid stomping settings
    fn save_to_disk(&self, state: &StoredState) -> Result<(), AppError> {
        let mut obj: serde_json::Map<String, serde_json::Value> =
            if let serde_json::Value::Object(map) = self.file.read_raw()? {
                map
            } else {
                serde_json::Map::new()
            };
        obj.insert(
            "projects".to_string(),
            serde_json::to_value(&state.projects)?,
        );
        obj.insert(
            "nextPort".to_string(),
            serde_json::to_value(state.next_port)?,
        );
        obj.insert(
            "snapshots".to_string(),
            serde_json::to_value(&state.snapshots)?,
        );
        self.file.save(&serde_json::Value::Object(obj))
    }
}

impl ProjectStore for JsonProjectStore {
    fn list(&self) -> Result<Vec<Project>, AppError> {
        let state = self.state.lock()?;
        Ok(state.projects.clone())
    }

    fn get(&self, id: &ProjectId) -> Result<Project, AppError> {
        let state = self.state.lock()?;

        state
            .projects
            .iter()
            .find(|p| p.id == *id)
            .cloned()
            .ok_or_else(|| AppError::NotFound {
                entity: "project".into(),
                id: id.to_string(),
            })
    }

    fn add(&self, project: Project) -> Result<(), AppError> {
        let mut state = self.state.lock()?;

        // Check for duplicates
        if state.projects.iter().any(|p| p.id == project.id) {
            return Err(AppError::AlreadyExists {
                entity: "project".into(),
                id: project.id.to_string(),
            });
        }

        state.projects.push(project);
        self.save_to_disk(&state)
    }

    fn update(&self, project: Project) -> Result<(), AppError> {
        let mut state = self.state.lock()?;

        let existing = state
            .projects
            .iter_mut()
            .find(|p| p.id == project.id)
            .ok_or_else(|| AppError::NotFound {
                entity: "project".into(),
                id: project.id.to_string(),
            })?;

        *existing = project;
        self.save_to_disk(&state)
    }

    fn remove(&self, id: &ProjectId) -> Result<(), AppError> {
        let mut state = self.state.lock()?;

        let original_len = state.projects.len();
        state.projects.retain(|p| p.id != *id);

        if state.projects.len() == original_len {
            return Err(AppError::NotFound {
                entity: "project".into(),
                id: id.to_string(),
            });
        }

        self.save_to_disk(&state)
    }

    fn allocate_port(&self) -> Result<u16, AppError> {
        let mut state = self.state.lock()?;

        // Collect ports currently claimed by projects
        let claimed: HashSet<u16> = state.projects.iter().filter_map(|p| p.port).collect();

        let range_size = u32::from(ports::MAX_PORT - ports::DEFAULT_NEXT_PORT) + 1;
        let mut attempts = 0u32;
        let mut port = state.next_port;

        // Skip ports already assigned to existing projects
        while claimed.contains(&port) {
            port = if port >= ports::MAX_PORT {
                ports::DEFAULT_NEXT_PORT
            } else {
                port + 1
            };
            attempts += 1;
            if attempts >= range_size {
                return Err(AppError::Internal {
                    reason: "cannot allocate port: all ports in range are claimed".into(),
                });
            }
        }

        // Advance next_port past the allocated one
        state.next_port = if port >= ports::MAX_PORT {
            ports::DEFAULT_NEXT_PORT
        } else {
            port + 1
        };

        self.save_to_disk(&state)?;
        Ok(port)
    }

    fn next_port(&self) -> Result<u16, AppError> {
        let state = self.state.lock()?;
        Ok(state.next_port)
    }

    fn add_snapshot(&self, record: crate::snapshots::SnapshotRecord) -> Result<(), AppError> {
        let mut state = self.state.lock()?;
        let project_id = record.project_id.clone();
        state.snapshots.push(record);
        // FIFO-evict oldest for this project until at-or-below cap.
        let mut count = state
            .snapshots
            .iter()
            .filter(|s| s.project_id == project_id)
            .count();
        while count > 20 {
            if let Some(idx) = state
                .snapshots
                .iter()
                .position(|s| s.project_id == project_id)
            {
                state.snapshots.remove(idx);
                count -= 1;
            } else {
                break;
            }
        }
        self.save_to_disk(&state)
    }

    fn list_snapshots(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<crate::snapshots::SnapshotRecord>, AppError> {
        let state = self.state.lock()?;
        Ok(state
            .snapshots
            .iter()
            .filter(|s| &s.project_id == project_id)
            .cloned()
            .collect())
    }

    fn remove_snapshot(&self, id: &crate::snapshots::SnapshotId) -> Result<(), AppError> {
        let mut state = self.state.lock()?;
        let before = state.snapshots.len();
        state.snapshots.retain(|s| &s.id != id);
        if state.snapshots.len() == before {
            return Err(AppError::NotFound {
                entity: "snapshot".into(),
                id: id.as_str().to_string(),
            });
        }
        self.save_to_disk(&state)
    }

    fn get_snapshot(
        &self,
        id: &crate::snapshots::SnapshotId,
    ) -> Result<crate::snapshots::SnapshotRecord, AppError> {
        let state = self.state.lock()?;
        state
            .snapshots
            .iter()
            .find(|s| &s.id == id)
            .cloned()
            .ok_or_else(|| AppError::NotFound {
                entity: "snapshot".into(),
                id: id.as_str().to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::ports;
    use crate::projects::types::Project;
    use tempfile::TempDir;

    fn make_project(id: &str, name: &str) -> Project {
        use crate::projects::types::{ProjectIntent, ProjectStatus};
        Project {
            id: ProjectId::new(id).unwrap(),
            name: name.to_string(),
            slug: None,
            repo_url: "https://example.com/repo".to_string(),
            local_path: format!("/tmp/{id}"),
            status: ProjectStatus::Stopped,
            intent: ProjectIntent::Stop,
            port: None,
            branch: "main".to_string(),
            last_synced_at: 0,
            tunnel_id: None,
            tunnel_url: None,
            pages_url: None,
            installed: false,
            parent_project_id: None,
            forked_at: None,
        }
    }

    #[test]
    fn test_project_store_trait_is_object_safe() {
        fn _accepts_dyn(_: &dyn ProjectStore) {}
    }

    #[test]
    fn test_project_store_default_next_port() {
        let state = StoredState::default();
        assert_eq!(state.next_port, ports::DEFAULT_NEXT_PORT);
    }

    #[test]
    fn test_project_store_backward_compatibility_missing_next_port() {
        let legacy = r#"{"projects":[],"settings":{"vmMemoryMb":2048}}"#;
        let state: StoredState = serde_json::from_str(legacy).unwrap();
        assert_eq!(state.next_port, ports::DEFAULT_NEXT_PORT);
    }

    #[test]
    fn test_projects_save_preserves_settings() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");

        // Seed file with settings (as if settings store wrote it)
        let initial: serde_json::Value = serde_json::json!({
            "settings": {
                "vmMemoryMb": 4096,
                "autoStartVm": true,
                "startOnLogin": true,
                "autoSyncEnabled": false,
                "syncDebounceSeconds": 90,
                "syncPushIntervalSeconds": 600,
                "syncPushOnStop": false
            },
            "projects": [],
            "nextPort": 3001
        });
        std::fs::write(&path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();

        let (store, _report) = JsonProjectStore::load_from_path(path.clone());
        store.add(make_project("proj-1", "My Project")).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let obj = content.as_object().unwrap();

        // Projects updated
        let projects = obj.get("projects").unwrap().as_array().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(
            projects
                .first()
                .unwrap()
                .get("name")
                .unwrap()
                .as_str()
                .unwrap(),
            "My Project"
        );

        // Unrelated sections preserved
        let settings = obj.get("settings").unwrap().as_object().unwrap();
        assert_eq!(settings.get("vmMemoryMb").unwrap().as_u64().unwrap(), 4096);
        assert_eq!(
            settings
                .get("syncDebounceSeconds")
                .unwrap()
                .as_u64()
                .unwrap(),
            90
        );
    }

    #[test]
    fn test_allocate_port_skips_claimed() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let (store, _report) = JsonProjectStore::load_from_path(path);

        // Add a project with a claimed port
        let mut proj = make_project("proj-1", "Test");
        proj.port = Some(ports::DEFAULT_NEXT_PORT);
        store.add(proj).unwrap();

        // allocate_port should skip the claimed port
        let port = store.allocate_port().unwrap();
        assert_ne!(port, ports::DEFAULT_NEXT_PORT);
        assert_eq!(port, ports::DEFAULT_NEXT_PORT + 1);
    }

    #[test]
    fn get_by_slug_returns_matching_project() {
        let dir = TempDir::new().unwrap();
        let (store, _report) = JsonProjectStore::load_from_path(dir.path().join("state.json"));

        let mut proj = make_project("proj-1", "Test");
        proj.slug = Some("my-blog".to_string());
        store.add(proj).unwrap();

        let found = store.get_by_slug("my-blog").unwrap();
        assert_eq!(found.id.as_str(), "proj-1");
        assert_eq!(found.slug.as_deref(), Some("my-blog"));
    }

    #[test]
    fn get_by_slug_returns_not_found_when_missing() {
        let dir = TempDir::new().unwrap();
        let (store, _report) = JsonProjectStore::load_from_path(dir.path().join("state.json"));

        let mut proj = make_project("proj-1", "Test");
        proj.slug = Some("my-blog".to_string());
        store.add(proj).unwrap();

        let err = store.get_by_slug("missing").unwrap_err();
        assert!(
            matches!(
                &err,
                AppError::NotFound { entity, id } if entity == "project" && id == "missing"
            ),
            "expected NotFound{{entity:\"project\", id:\"missing\"}}, got {err:?}",
        );
    }

    #[test]
    fn load_backfills_slugs_for_legacy_slugless_projects() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let legacy = serde_json::json!({
            "projects": [
                {
                    "id": "proj-1",
                    "name": "Agentic Conference",
                    "repoUrl": "https://github.com/vtemian/agentic-conference.git",
                    "localPath": "/tmp/proj-1",
                    "status": "stopped",
                    "port": null,
                    "branch": "main",
                    "lastSyncedAt": 0
                },
                {
                    "id": "proj-2",
                    "name": "Other",
                    "repoUrl": "https://github.com/vtemian/agentic-conference.git",
                    "localPath": "/tmp/proj-2",
                    "status": "stopped",
                    "port": null,
                    "branch": "main",
                    "lastSyncedAt": 0
                }
            ],
            "nextPort": 3001,
            "settings": {}
        });
        std::fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

        let (store, _report) = JsonProjectStore::load_from_path(path.clone());

        let p1 = store.get_by_slug("agentic-conference").unwrap();
        assert_eq!(p1.id.as_str(), "proj-1");
        let p2 = store.get_by_slug("agentic-conference-2").unwrap();
        assert_eq!(p2.id.as_str(), "proj-2");

        // The assigned slugs were persisted, so a fresh load reads them back
        // unchanged rather than re-deriving.
        let (reloaded, _report) = JsonProjectStore::load_from_path(path);
        let reloaded_p1 = reloaded.get(&ProjectId::new("proj-1").unwrap()).unwrap();
        assert_eq!(reloaded_p1.slug.as_deref(), Some("agentic-conference"));
    }

    #[test]
    fn load_backfill_preserves_existing_slugs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let seeded = serde_json::json!({
            "projects": [
                {
                    "id": "proj-1",
                    "name": "Already Linked",
                    "slug": "custom-slug",
                    "repoUrl": "https://github.com/owner/repo.git",
                    "localPath": "/tmp/proj-1",
                    "status": "stopped",
                    "port": null,
                    "branch": "main",
                    "lastSyncedAt": 0
                }
            ],
            "nextPort": 3001,
            "settings": {}
        });
        std::fs::write(&path, serde_json::to_string_pretty(&seeded).unwrap()).unwrap();

        let (store, _report) = JsonProjectStore::load_from_path(path);
        let found = store.get_by_slug("custom-slug").unwrap();
        assert_eq!(found.id.as_str(), "proj-1");
    }

    #[test]
    fn test_stored_state_backward_compat_missing_snapshots() {
        let legacy = r#"{"projects":[],"nextPort":3001,"settings":{"vmMemoryMb":2048}}"#;
        let state: StoredState = serde_json::from_str(legacy).unwrap();
        assert!(state.snapshots.is_empty());
    }

    #[test]
    fn test_save_to_disk_persists_snapshots_field() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let (store, _) = JsonProjectStore::load_from_path(path.clone());
        let parent_id = crate::projects::ProjectId::new("proj-parent").unwrap();
        let record = crate::snapshots::SnapshotRecord {
            id: crate::snapshots::SnapshotId::new("snap-1-aaaa").unwrap(),
            project_id: parent_id.clone(),
            label: Some("before-refactor".into()),
            taken_at: 1_234_567_890,
            size_bytes: 999,
        };
        store.add_snapshot(record).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        let arr = json
            .get("snapshots")
            .expect("snapshots key")
            .as_array()
            .expect("array");
        assert_eq!(arr.len(), 1);
    }

    #[test]
    fn add_snapshot_evicts_oldest_at_cap_per_project() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("state.json");
        let (store, _) = JsonProjectStore::load_from_path(path);
        let proj_a = crate::projects::ProjectId::new("proj-a").unwrap();
        let proj_b = crate::projects::ProjectId::new("proj-b").unwrap();

        // Fill A with 20 snapshots (snap-a-00 .. snap-a-19), B with 5.
        for i in 0u64..20 {
            store
                .add_snapshot(crate::snapshots::SnapshotRecord {
                    id: crate::snapshots::SnapshotId::new(format!("snap-a-{i:02}")).unwrap(),
                    project_id: proj_a.clone(),
                    label: None,
                    taken_at: i,
                    size_bytes: 0,
                })
                .unwrap();
        }
        for i in 0u64..5 {
            store
                .add_snapshot(crate::snapshots::SnapshotRecord {
                    id: crate::snapshots::SnapshotId::new(format!("snap-b-{i:02}")).unwrap(),
                    project_id: proj_b.clone(),
                    label: None,
                    taken_at: i,
                    size_bytes: 0,
                })
                .unwrap();
        }
        // 21st A snapshot should evict the oldest A snapshot (snap-a-00),
        // leaving B's 5 untouched.
        store
            .add_snapshot(crate::snapshots::SnapshotRecord {
                id: crate::snapshots::SnapshotId::new("snap-a-20").unwrap(),
                project_id: proj_a.clone(),
                label: None,
                taken_at: 20,
                size_bytes: 0,
            })
            .unwrap();

        let a_list = store.list_snapshots(&proj_a).unwrap();
        assert_eq!(a_list.len(), 20);
        assert!(
            a_list.iter().all(|s| s.id.as_str() != "snap-a-00"),
            "snap-a-00 should have been evicted"
        );
        assert!(a_list.iter().any(|s| s.id.as_str() == "snap-a-20"));
        assert_eq!(store.list_snapshots(&proj_b).unwrap().len(), 5);
    }

    #[test]
    fn remove_snapshot_returns_not_found_when_missing() {
        let dir = TempDir::new().unwrap();
        let (store, _) = JsonProjectStore::load_from_path(dir.path().join("state.json"));
        let id = crate::snapshots::SnapshotId::new("snap-missing").unwrap();
        let err = store.remove_snapshot(&id).unwrap_err();
        assert!(
            matches!(&err, AppError::NotFound { entity, id: eid } if entity == "snapshot" && eid == "snap-missing"),
            "expected NotFound, got {err:?}"
        );
    }

    #[test]
    fn get_snapshot_returns_record() {
        let dir = TempDir::new().unwrap();
        let (store, _) = JsonProjectStore::load_from_path(dir.path().join("state.json"));
        let proj = crate::projects::ProjectId::new("proj-a").unwrap();
        let record = crate::snapshots::SnapshotRecord {
            id: crate::snapshots::SnapshotId::new("snap-only").unwrap(),
            project_id: proj.clone(),
            label: Some("label".into()),
            taken_at: 42,
            size_bytes: 7,
        };
        store.add_snapshot(record).unwrap();
        let got = store
            .get_snapshot(&crate::snapshots::SnapshotId::new("snap-only").unwrap())
            .unwrap();
        assert_eq!(got.id.as_str(), "snap-only");
        assert_eq!(got.label.as_deref(), Some("label"));
    }

    #[test]
    fn get_by_slug_skips_projects_with_no_slug() {
        let dir = TempDir::new().unwrap();
        let (store, _report) = JsonProjectStore::load_from_path(dir.path().join("state.json"));

        // proj-1 has no slug, proj-2 does
        store.add(make_project("proj-1", "Test 1")).unwrap();
        let mut proj_2 = make_project("proj-2", "Test 2");
        proj_2.slug = Some("the-blog".to_string());
        store.add(proj_2).unwrap();

        // Looking up by an empty-equivalent slug must not match the None-slug project
        assert!(store.get_by_slug("").is_err());

        let found = store.get_by_slug("the-blog").unwrap();
        assert_eq!(found.id.as_str(), "proj-2");
    }
}
