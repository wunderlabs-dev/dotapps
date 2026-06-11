//! Recovery report returned by stores when a load involved corruption handling.

use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum RecoveryReport {
    RecoveredFromBackup,
    ArchivedAndReset { archive_paths: Vec<PathBuf> },
}
