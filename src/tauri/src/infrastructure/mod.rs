//! Infrastructure layer: external system integrations
//!
//! This module provides trait abstractions and implementations for:
//! - Container runtime (Podman/Docker via bollard)
//! - Git operations (via libgit2)
//! - OS keychain (via keyring crate, with in-memory cache)

pub mod atomic_json;
pub mod git;
pub mod keychain;
pub mod recovery;
pub mod runtime;

pub use self::atomic_json::{AtomicJsonFile, LoadOutcome};
pub use self::git::{
    default_commit_message, CloneResult, CommitAutoResult, GitOps, LibGitClient, PushOutcome,
    RepoSyncStatus,
};
pub use self::keychain::{Keychain, OsKeychain};
pub use self::recovery::RecoveryReport;
#[cfg(target_os = "linux")]
pub use self::runtime::PodmanRuntime;
pub use self::runtime::Runtime;
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub use self::runtime::StubRuntime;
