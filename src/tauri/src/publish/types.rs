//! Publish-related types: framework detection, build configuration

use serde::Serialize;

/// Detected JavaScript framework
#[derive(Clone, Debug, Eq, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Framework {
    NextJs,
    Vite,
    Cra,
    Unknown,
}

/// Build configuration derived from framework detection
#[derive(Clone, Debug)]
pub struct BuildConfig {
    pub framework: Framework,
    /// Shell command to run inside the container (e.g. "npm install && npx next build")
    pub build_cmd: String,
    /// Directory containing the static build output, relative to repo root (e.g. "out")
    pub output_dir: String,
}

/// Parsed GitHub owner/repo from a git URL
#[derive(Clone, Debug)]
pub struct RepoIdentity {
    pub owner: String,
    pub repo: String,
}

/// Response returned to the frontend after publishing
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PublishResponse {
    pub url: String,
}
