//! Typed publish failure modes.
//!
//! Wrapped by [`crate::error::AppError::Publish`] so MCP tools and the
//! frontend can branch on a discriminator instead of regex-matching
//! formatted strings.

use serde::Serialize;
use thiserror::Error;

/// Discriminated publish failure variants.
///
/// Serialized with `tag = "kind", content = "detail"` so the wire shape is
/// `{"kind": "...", "detail": {...}}` (or just `{"kind": "..."}` for unit
/// variants), nested inside `AppError::Publish`'s `detail` field.
#[derive(Clone, Debug, Error, Serialize, specta::Type)]
#[serde(tag = "kind", content = "detail")]
pub enum PublishError {
    #[error("no package.json found in project root")]
    NoPackageJson,

    #[error("no known framework detected and no 'build' script in package.json")]
    NoBuildScript,

    #[error(
        "Next.js project is not configured for static export. Add `output: 'export'` \
         to your next.config file to publish to GitHub Pages."
    )]
    NextjsStaticExportRequired,

    #[error(
        "GitHub Pages requires a Pro plan for private repositories. \
         Make the repository public or upgrade your GitHub plan."
    )]
    PrivateRepoNeedsPro,

    #[error(
        "cannot publish: build environment is not ready. \
         Start any project first to initialize the runtime, then try publishing."
    )]
    RuntimeNotReady,

    #[error("build failed:\n{tail}")]
    BuildFailed { tail: String },

    #[error("build completed but output directory '{expected}' not found")]
    BuildOutputMissing { expected: String },

    #[error("cannot push to gh-pages: {reason}")]
    PushFailed { reason: String },

    #[error("GitHub Pages API failed (HTTP {status}): {body}")]
    PagesApiFailed { status: u16, body: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_variant_serializes_with_kind_only() {
        let json = serde_json::to_value(PublishError::NoPackageJson).expect("serialize");
        assert_eq!(json, serde_json::json!({"kind": "NoPackageJson"}));
    }

    #[test]
    fn struct_variant_serializes_with_kind_and_detail() {
        let json = serde_json::to_value(PublishError::BuildFailed {
            tail: "npm ERR! exited 1".into(),
        })
        .expect("serialize");
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "BuildFailed",
                "detail": {"tail": "npm ERR! exited 1"},
            })
        );
    }

    #[test]
    fn pages_api_failed_carries_status_and_body() {
        let json = serde_json::to_value(PublishError::PagesApiFailed {
            status: 404,
            body: "not found".into(),
        })
        .expect("serialize");
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "PagesApiFailed",
                "detail": {"status": 404, "body": "not found"},
            })
        );
    }
}
