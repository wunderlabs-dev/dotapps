//! Atomic JSON file persistence with backup-and-recover semantics.
//!
//! Save writes to a temp file, rotates the existing primary to `.bak`,
//! and renames the temp file in place so a torn write never wipes data.
//! Load tries primary, falls back to `.bak`, and archives both with a
//! timestamp suffix when neither parses.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::AppError;

pub struct AtomicJsonFile {
    path: PathBuf,
}

pub enum LoadOutcome<T> {
    /// File did not exist; caller should treat the value as default.
    Fresh(T),
    /// Primary parsed cleanly.
    Loaded(T),
    /// Primary was unreadable or unparseable; the `.bak` was used and
    /// has been promoted back to primary.
    RecoveredFromBackup(T),
    /// Both primary and backup were unparseable. Both were renamed with
    /// a `.corrupted-{unix_ts}` suffix; caller starts from default.
    ArchivedAndReset { archive_paths: Vec<PathBuf> },
}

impl AtomicJsonFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn backup_path(&self) -> PathBuf {
        self.path.with_extension("json.bak")
    }

    fn temp_path(&self) -> PathBuf {
        self.path.with_extension("json.tmp")
    }

    /// Load with corruption recovery. T must implement Default for the
    /// fresh-and-archived cases.
    pub fn load<T: DeserializeOwned + Default>(&self) -> LoadOutcome<T> {
        if !self.path.exists() {
            return LoadOutcome::Fresh(T::default());
        }

        match Self::try_parse(&self.path) {
            Ok(value) => LoadOutcome::Loaded(value),
            Err(primary_err) => {
                tracing::error!(
                    "cannot parse {}: {primary_err}; trying backup",
                    self.path.display()
                );
                self.recover_from_backup::<T>()
            }
        }
    }

    fn recover_from_backup<T: DeserializeOwned + Default>(&self) -> LoadOutcome<T> {
        let backup = self.backup_path();
        if backup.exists() {
            match Self::try_parse::<T>(&backup) {
                Ok(value) => {
                    // Promote backup back to primary so next save has a
                    // good baseline to rotate from.
                    if let Err(e) = fs::copy(&backup, &self.path) {
                        tracing::warn!(
                            "cannot promote backup to primary: {e}; \
                             will return value but next save may rotate it out"
                        );
                    }
                    return LoadOutcome::RecoveredFromBackup(value);
                }
                Err(backup_err) => {
                    tracing::error!("cannot parse backup {}: {backup_err}", backup.display());
                }
            }
        }

        // Both gone or corrupt: archive whatever exists and start fresh.
        let mut archived = Vec::new();
        let ts = unix_timestamp();
        for path in [&self.path, &backup] {
            if path.exists() {
                let archive = path.with_extension(format!("json.corrupted-{ts}"));
                if let Err(e) = fs::rename(path, &archive) {
                    tracing::error!("cannot archive {}: {e}", path.display());
                } else {
                    archived.push(archive);
                }
            }
        }

        LoadOutcome::ArchivedAndReset {
            archive_paths: archived,
        }
    }

    fn try_parse<T: DeserializeOwned>(path: &Path) -> Result<T, AppError> {
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Save with backup rotation. Always writes pretty-printed JSON.
    pub fn save<T: Serialize>(&self, value: &T) -> Result<(), AppError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(value)?;
        let tmp = self.temp_path();
        fs::write(&tmp, &content)?;

        // Rotate current primary to backup before swapping in tmp.
        // Ignore "primary doesn't exist yet": that's a fresh install.
        if self.path.exists() {
            let backup = self.backup_path();
            if let Err(e) = fs::rename(&self.path, &backup) {
                tracing::warn!("cannot rotate primary to backup ({e}); proceeding with rename");
            }
        }

        fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    /// Read the raw on-disk JSON as a `serde_json::Value`, with the same
    /// recovery flow as `load`. Used by stores that merge unrelated
    /// fields and need the raw object map, not a typed struct.
    pub fn read_raw(&self) -> Result<serde_json::Value, AppError> {
        match self.load::<serde_json::Value>() {
            LoadOutcome::Fresh(_) | LoadOutcome::ArchivedAndReset { .. } => {
                Ok(serde_json::Value::Object(serde_json::Map::new()))
            }
            LoadOutcome::Loaded(v) | LoadOutcome::RecoveredFromBackup(v) => Ok(v),
        }
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
#[expect(
    clippy::panic,
    reason = "test-only refutable-let-else branches must panic to fail the test"
)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    struct Sample {
        name: String,
        count: u32,
    }

    fn fresh(dir: &TempDir) -> AtomicJsonFile {
        AtomicJsonFile::new(dir.path().join("state.json"))
    }

    #[test]
    fn load_returns_fresh_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let file = fresh(&dir);
        let outcome = file.load::<Sample>();
        assert!(matches!(outcome, LoadOutcome::Fresh(_)));
    }

    #[test]
    fn save_then_load_returns_loaded() {
        let dir = TempDir::new().unwrap();
        let file = fresh(&dir);
        file.save(&Sample {
            name: "vlad".into(),
            count: 7,
        })
        .unwrap();
        let LoadOutcome::Loaded(v) = file.load::<Sample>() else {
            panic!("expected Loaded");
        };
        assert_eq!(
            v,
            Sample {
                name: "vlad".into(),
                count: 7
            }
        );
    }

    #[test]
    fn save_creates_backup_from_prior_primary() {
        let dir = TempDir::new().unwrap();
        let file = fresh(&dir);
        file.save(&Sample {
            name: "a".into(),
            count: 1,
        })
        .unwrap();
        file.save(&Sample {
            name: "b".into(),
            count: 2,
        })
        .unwrap();

        let backup = dir.path().join("state.json.bak");
        assert!(backup.exists(), "backup should have been created");

        let backup_content = std::fs::read_to_string(&backup).unwrap();
        let parsed: Sample = serde_json::from_str(&backup_content).unwrap();
        assert_eq!(parsed.name, "a", "backup should hold previous version");
    }

    #[test]
    fn corrupt_primary_falls_back_to_backup() {
        let dir = TempDir::new().unwrap();
        let file = fresh(&dir);
        file.save(&Sample {
            name: "good".into(),
            count: 1,
        })
        .unwrap();
        file.save(&Sample {
            name: "newer".into(),
            count: 2,
        })
        .unwrap();

        // Corrupt the primary; backup still has "good"
        std::fs::write(file.path(), b"{ broken json").unwrap();

        let LoadOutcome::RecoveredFromBackup(v) = file.load::<Sample>() else {
            panic!("expected RecoveredFromBackup");
        };
        assert_eq!(v.name, "good");

        // Promotion: primary should now match backup again
        let primary = std::fs::read_to_string(file.path()).unwrap();
        let parsed: Sample = serde_json::from_str(&primary).unwrap();
        assert_eq!(parsed.name, "good");
    }

    #[test]
    fn both_corrupt_archives_and_resets() {
        let dir = TempDir::new().unwrap();
        let file = fresh(&dir);
        file.save(&Sample {
            name: "a".into(),
            count: 1,
        })
        .unwrap();
        file.save(&Sample {
            name: "b".into(),
            count: 2,
        })
        .unwrap();

        std::fs::write(file.path(), b"{ broken").unwrap();
        std::fs::write(dir.path().join("state.json.bak"), b"{ also broken").unwrap();

        let LoadOutcome::ArchivedAndReset { archive_paths } = file.load::<Sample>() else {
            panic!("expected ArchivedAndReset");
        };
        assert_eq!(archive_paths.len(), 2);
        for p in &archive_paths {
            assert!(p.exists());
            assert!(p.to_string_lossy().contains(".corrupted-"));
        }
        assert!(!file.path().exists(), "primary should be moved aside");
    }

    #[test]
    fn save_writes_atomically_via_temp_rename() {
        // No good way to crash mid-save in a unit test, but verify
        // that no .tmp file is left behind after a successful save.
        let dir = TempDir::new().unwrap();
        let file = fresh(&dir);
        file.save(&Sample {
            name: "x".into(),
            count: 1,
        })
        .unwrap();
        assert!(!dir.path().join("state.json.tmp").exists());
    }
}
