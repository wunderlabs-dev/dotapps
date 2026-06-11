//! Installed-app storage at `~/.dotapps/apps.json`.
#![expect(
    clippy::disallowed_types,
    reason = "sync file I/O locking, not held across await points"
)]

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use super::types::InstalledApp;
use crate::error::AppError;

/// First host port assigned to a dotapps app. Project dev servers allocate from
/// 3001, so apps get their own range to avoid collisions.
const FIRST_PORT: u16 = 4100;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct StoredApps {
    /// slug -> installed app
    #[serde(default)]
    apps: HashMap<String, InstalledApp>,
}

/// JSON-backed store of installed dotapps apps, keyed by slug.
///
/// Deliberately separate from `JsonProjectStore`: apps and projects share no
/// state, and the store stays minimal (single file, atomic rename on save).
pub struct AppStore {
    path: PathBuf,
    state: Mutex<StoredApps>,
}

impl AppStore {
    /// Load the store from `path`. Missing or unparsable files fall back to
    /// empty: a corrupt file means reinstalling apps, never a crash.
    pub fn load(path: PathBuf) -> Self {
        let state = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            state: Mutex::new(state),
        }
    }

    /// List installed apps sorted by display name.
    pub fn list(&self) -> Result<Vec<InstalledApp>, AppError> {
        let state = self.state.lock()?;
        let mut apps: Vec<InstalledApp> = state.apps.values().cloned().collect();
        apps.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
        Ok(apps)
    }

    /// Get an installed app by slug.
    pub fn get(&self, slug: &str) -> Result<Option<InstalledApp>, AppError> {
        let state = self.state.lock()?;
        Ok(state.apps.get(slug).cloned())
    }

    /// Insert or replace the app keyed by its manifest slug.
    pub fn upsert(&self, app: InstalledApp) -> Result<(), AppError> {
        let mut state = self.state.lock()?;
        state.apps.insert(app.manifest.slug.clone(), app);
        self.save(&state)
    }

    /// Flip the running flag for an installed app.
    pub fn set_running(&self, slug: &str, running: bool) -> Result<(), AppError> {
        let mut state = self.state.lock()?;
        let app = state
            .apps
            .get_mut(slug)
            .ok_or_else(|| not_installed(slug))?;
        app.running = running;
        self.save(&state)
    }

    /// Stable port per slug: reuse the assigned port if present, otherwise
    /// assign (and persist) the first free port >= [`FIRST_PORT`].
    pub fn allocate_port(&self, slug: &str) -> Result<u16, AppError> {
        let mut state = self.state.lock()?;
        let existing = state
            .apps
            .get(slug)
            .ok_or_else(|| not_installed(slug))?
            .host_port;
        if let Some(port) = existing {
            return Ok(port);
        }

        let claimed: HashSet<u16> = state.apps.values().filter_map(|a| a.host_port).collect();
        let mut port = FIRST_PORT;
        while claimed.contains(&port) {
            port = port.checked_add(1).ok_or_else(|| AppError::Internal {
                reason: "cannot allocate app port: range exhausted".into(),
            })?;
        }

        if let Some(app) = state.apps.get_mut(slug) {
            app.host_port = Some(port);
        }
        self.save(&state)?;
        Ok(port)
    }

    /// Write to a `.tmp` sibling then rename (atomic on the same volume).
    fn save(&self, state: &StoredApps) -> Result<(), AppError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(state)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

/// `NotFound` for a slug with no installed app. Shared with `apps::commands`.
pub(crate) fn not_installed(slug: &str) -> AppError {
    AppError::NotFound {
        entity: "app".into(),
        id: slug.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::types::Manifest;
    use tempfile::TempDir;

    fn make_app(slug: &str, name: &str) -> InstalledApp {
        InstalledApp {
            manifest: Manifest {
                name: name.to_string(),
                slug: slug.to_string(),
                version: "1.0.0".to_string(),
                icon: "📦".to_string(),
                internal_port: 8000,
                description: String::new(),
            },
            host_port: None,
            running: false,
        }
    }

    #[test]
    fn upsert_and_list_round_trip_through_disk() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("apps.json");

        let store = AppStore::load(path.clone());
        store.upsert(make_app("shift-board", "Shift Board")).unwrap();
        store.upsert(make_app("cafe-tracker", "Cafe Tracker")).unwrap();

        let reloaded = AppStore::load(path);
        let apps = reloaded.list().unwrap();
        assert_eq!(apps.len(), 2);
        // Sorted by display name
        assert_eq!(apps.first().unwrap().manifest.slug, "cafe-tracker");
        assert_eq!(apps.get(1).unwrap().manifest.slug, "shift-board");
    }

    #[test]
    fn upsert_replaces_existing_slug() {
        let dir = TempDir::new().unwrap();
        let store = AppStore::load(dir.path().join("apps.json"));

        store.upsert(make_app("cafe-tracker", "Cafe Tracker")).unwrap();
        let mut updated = make_app("cafe-tracker", "Cafe Tracker");
        updated.manifest.version = "2.0.0".to_string();
        store.upsert(updated).unwrap();

        let apps = store.list().unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps.first().unwrap().manifest.version, "2.0.0");
    }

    #[test]
    fn allocate_port_starts_at_4100_and_is_stable_per_slug() {
        let dir = TempDir::new().unwrap();
        let store = AppStore::load(dir.path().join("apps.json"));
        store.upsert(make_app("cafe-tracker", "Cafe Tracker")).unwrap();
        store.upsert(make_app("shift-board", "Shift Board")).unwrap();

        assert_eq!(store.allocate_port("cafe-tracker").unwrap(), 4100);
        assert_eq!(store.allocate_port("shift-board").unwrap(), 4101);
        // Repeat calls return the same port for the same slug
        assert_eq!(store.allocate_port("cafe-tracker").unwrap(), 4100);
        assert_eq!(store.allocate_port("shift-board").unwrap(), 4101);
    }

    #[test]
    fn allocate_port_persists_across_reload() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("apps.json");

        let store = AppStore::load(path.clone());
        store.upsert(make_app("cafe-tracker", "Cafe Tracker")).unwrap();
        assert_eq!(store.allocate_port("cafe-tracker").unwrap(), 4100);

        let reloaded = AppStore::load(path);
        assert_eq!(reloaded.allocate_port("cafe-tracker").unwrap(), 4100);
    }

    #[test]
    fn allocate_port_unknown_slug_is_not_found() {
        let dir = TempDir::new().unwrap();
        let store = AppStore::load(dir.path().join("apps.json"));
        let err = store.allocate_port("ghost").unwrap_err();
        assert!(
            matches!(&err, AppError::NotFound { entity, id } if entity == "app" && id == "ghost"),
            "expected NotFound, got {err:?}"
        );
    }

    #[test]
    fn set_running_updates_flag_and_persists() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("apps.json");

        let store = AppStore::load(path.clone());
        store.upsert(make_app("cafe-tracker", "Cafe Tracker")).unwrap();
        store.set_running("cafe-tracker", true).unwrap();

        let reloaded = AppStore::load(path);
        let app = reloaded.get("cafe-tracker").unwrap().unwrap();
        assert!(app.running);
    }

    #[test]
    fn set_running_unknown_slug_is_not_found() {
        let dir = TempDir::new().unwrap();
        let store = AppStore::load(dir.path().join("apps.json"));
        assert!(store.set_running("ghost", true).is_err());
    }

    #[test]
    fn load_missing_or_corrupt_file_is_empty() {
        let dir = TempDir::new().unwrap();

        let missing = AppStore::load(dir.path().join("missing.json"));
        assert!(missing.list().unwrap().is_empty());

        let corrupt_path = dir.path().join("corrupt.json");
        std::fs::write(&corrupt_path, b"{ not json").unwrap();
        let corrupt = AppStore::load(corrupt_path);
        assert!(corrupt.list().unwrap().is_empty());
    }
}
