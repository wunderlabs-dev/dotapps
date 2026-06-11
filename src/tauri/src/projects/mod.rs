//! Projects feature module
//!
//! Manages project lifecycle: CRUD, import from git, start/stop containers.

pub mod commands;
pub mod cursor;
pub mod importer;
pub mod marker;
pub mod orchestrator;
pub mod runner;
pub mod slug;
pub mod store;
pub mod sync;
pub mod syncer;
pub mod types;

pub use importer::ProjectImporter;
pub use orchestrator::ProjectOrchestrator;
pub use store::{JsonProjectStore, ProjectStore};
pub use sync::{SyncEvent, SyncPolicy, SyncState};
pub use syncer::ProjectSyncer;
#[expect(unused_imports, reason = "re-exported for external consumers")]
pub use types::{
    unix_now_millis, InstallStep, InstallStepEvent, InstallStepStatus, Project, ProjectId,
    ProjectIdError, ProjectIntent, ProjectStatus,
};
