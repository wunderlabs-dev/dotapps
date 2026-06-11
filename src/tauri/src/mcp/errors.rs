//! `AppError` -> `rmcp::ErrorData` mapping.
//!
//! Two-layer error model. The JSON-RPC numeric code (`-32602` for input
//! problems, `-32603` for internal failures) is what generic MCP clients
//! react to. The string `code` inside `data` is the stable contract for
//! Opnble-aware tools / docs: it never changes shape after release.
//!
//! Every variant gets an explicit arm: `wildcard_enum_match_arm` is denied
//! workspace-wide so the compiler forces us to update this file whenever a
//! new `AppError` variant lands.

#![allow(
    dead_code,
    reason = "Phase 2 wires `into_mcp` and `INTERNAL_ERROR`; some PublishError + AuthFailed paths are still unused until Phase 5/6 lifecycle tools land"
)]

use rmcp::model::{ErrorCode, ErrorData as McpError};
use serde_json::json;

use crate::error::AppError;
use crate::publish::PublishError;

/// JSON-RPC "Invalid params" code. Used for caller-fixable input issues:
/// missing slug, malformed id, already-exists collision.
const INVALID_PARAMS: ErrorCode = ErrorCode(-32602);

/// JSON-RPC "Internal error" code. Used for everything else: VM down, git
/// failure, container failure, auth-not-configured, etc.
const INTERNAL_ERROR: ErrorCode = ErrorCode(-32603);

/// Build the canonical "no GitHub credentials available" MCP error.
///
/// Reached when [`crate::auth::Authenticator::resolve_push_token`] returns
/// `Ok(None)` (the repo URL has no recognizable provider, so no token was
/// even requested). Tools that hit this path cannot fall through to the
/// usual `into_mcp(AppError::AuthRequired)` branch because they never get
/// an `AppError`. The shape mirrors what `into_mcp` produces for
/// `AppError::AuthRequired { provider: "github" }` so agents see one stable
/// `NO_GITHUB_AUTH` contract regardless of how the missing token was
/// detected. Shared by Phase 5 (preview link) and Phase 6 (publish/push).
pub fn no_github_auth() -> McpError {
    McpError {
        code: INTERNAL_ERROR,
        message: "cannot authenticate with github: login required".into(),
        data: Some(json!({
            "code": "NO_GITHUB_AUTH",
            "hint": "Open Opnble, then Settings, then GitHub",
        })),
    }
}

/// Map an [`AppError`] to the rmcp on-wire error shape.
pub fn into_mcp(err: AppError) -> McpError {
    let mapped = map_app_error(err);
    let MappedError {
        numeric,
        code,
        message,
        hint,
    } = mapped;

    let data = match hint {
        Some(h) => json!({ "code": code, "hint": h }),
        None => json!({ "code": code }),
    };

    McpError {
        code: numeric,
        message: message.into(),
        data: Some(data),
    }
}

/// `github_push` called on a project whose worktree has no `origin` remote
/// AND `create_repo_if_missing == false`. Surfaced as `INVALID_PARAMS`
/// because the caller can fix it by retrying with the flag set, or by
/// configuring a remote manually.
pub fn no_remote(slug: &str) -> McpError {
    McpError {
        code: INVALID_PARAMS,
        message: format!("cannot push '{slug}': no 'origin' remote configured").into(),
        data: Some(json!({
            "code": "NO_REMOTE",
            "hint": "retry with createRepoIfMissing=true to create a GitHub repo, or configure 'origin' on the worktree first",
        })),
    }
}

/// `github_push` performed the push but libgit2 reported a remote rejection
/// (auth, non-fast-forward, network). Pulled out so call sites stay terse.
pub fn push_failed(slug: &str, reason: &str) -> McpError {
    McpError {
        code: INTERNAL_ERROR,
        message: format!("cannot push '{slug}': {reason}").into(),
        data: Some(json!({
            "code": "PUSH_FAILED",
            "hint": reason,
        })),
    }
}

/// `github_pages_publish` exceeded the wall-clock ceiling. Distinct from a
/// `BUILD_FAILED` because the build may still be running on the runtime
/// (we just stopped waiting).
pub fn publish_timeout(slug: &str, secs: u64) -> McpError {
    McpError {
        code: INTERNAL_ERROR,
        message: format!("cannot publish '{slug}': build did not finish within {secs}s").into(),
        data: Some(json!({
            "code": "PUBLISH_TIMEOUT",
            "hint": "Build exceeded the 10-minute ceiling. Check the project's logs and try again.",
        })),
    }
}

/// Intermediate representation produced by the exhaustive match below. Keeps
/// `into_mcp` agnostic to whether a hint was provided.
struct MappedError {
    numeric: ErrorCode,
    code: &'static str,
    message: String,
    hint: Option<String>,
}

#[expect(
    clippy::too_many_lines,
    reason = "exhaustive match over the AppError surface; splitting hurts readability"
)]
fn map_app_error(err: AppError) -> MappedError {
    match err {
        AppError::NotFound { entity, id } => {
            let message = format!("cannot find {entity} '{id}'");
            if entity == "project" {
                MappedError {
                    numeric: INVALID_PARAMS,
                    code: "PROJECT_NOT_FOUND",
                    message,
                    hint: Some(id),
                }
            } else {
                MappedError {
                    numeric: INVALID_PARAMS,
                    code: "NOT_FOUND",
                    message,
                    hint: Some(format!("{entity}: {id}")),
                }
            }
        }
        AppError::AlreadyExists { entity, id } => {
            let message = format!("cannot create {entity} '{id}': already exists");
            MappedError {
                numeric: INVALID_PARAMS,
                code: "ALREADY_EXISTS",
                message,
                hint: Some(format!("{entity}: {id}")),
            }
        }
        AppError::InvalidInput { field, reason } => {
            let message = format!("cannot validate '{field}': {reason}");
            MappedError {
                numeric: INVALID_PARAMS,
                code: "INVALID_INPUT",
                message,
                hint: Some(format!("{field}: {reason}")),
            }
        }
        AppError::AuthRequired { provider } => {
            let message = format!("cannot authenticate with {provider}: login required");
            if provider == "github" {
                MappedError {
                    numeric: INTERNAL_ERROR,
                    code: "NO_GITHUB_AUTH",
                    message,
                    hint: Some("Open Opnble, then Settings, then GitHub".into()),
                }
            } else {
                MappedError {
                    numeric: INTERNAL_ERROR,
                    code: "NO_AUTH",
                    message,
                    hint: Some(format!("provider: {provider}")),
                }
            }
        }
        AppError::AuthFailed { reason } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "AUTH_FAILED",
            message: format!("cannot authenticate: {reason}"),
            hint: Some(reason),
        },
        AppError::ContainerFailed { reason } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "CONTAINER_FAILED",
            message: format!("cannot reach container runtime: {reason}"),
            hint: Some(reason),
        },
        AppError::GitFailed { reason } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "GIT_FAILED",
            message: format!("cannot complete git operation: {reason}"),
            hint: Some(reason),
        },
        AppError::StorageFailed { reason } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "STORAGE_FAILED",
            message: format!("cannot access storage: {reason}"),
            hint: Some(reason),
        },
        AppError::VmNotRunning => MappedError {
            numeric: INTERNAL_ERROR,
            code: "VM_OFFLINE",
            message: "cannot reach vm: not running".into(),
            hint: Some("Start any project from Opnble to bring up the VM".into()),
        },
        AppError::VmStartFailed { reason } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "VM_FAILED",
            message: format!("cannot start vm: {reason}"),
            hint: Some(reason),
        },
        AppError::VmStopFailed { reason } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "VM_FAILED",
            message: format!("cannot stop vm: {reason}"),
            hint: Some(reason),
        },
        AppError::VmConnectionFailed { reason } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "VM_FAILED",
            message: format!("cannot connect to vm: {reason}"),
            hint: Some(reason),
        },
        AppError::VmSetupFailed { reason } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "VM_FAILED",
            message: format!("cannot set up vm: {reason}"),
            hint: Some(reason),
        },
        AppError::TunnelFailed { reason } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "TUNNEL_FAILED",
            message: format!("cannot manage tunnel: {reason}"),
            hint: Some(reason),
        },
        AppError::Publish(publish_err) => map_publish_error(publish_err),
        AppError::Internal { reason } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "INTERNAL",
            message: reason.clone(),
            hint: Some(reason),
        },
    }
}

fn map_publish_error(err: PublishError) -> MappedError {
    let message = err.to_string();
    match err {
        PublishError::NoPackageJson => MappedError {
            numeric: INTERNAL_ERROR,
            code: "NO_PACKAGE_JSON",
            message,
            hint: None,
        },
        PublishError::NoBuildScript => MappedError {
            numeric: INTERNAL_ERROR,
            code: "NO_BUILD_SCRIPT",
            message,
            hint: None,
        },
        PublishError::NextjsStaticExportRequired => MappedError {
            numeric: INTERNAL_ERROR,
            code: "NEXTJS_STATIC_EXPORT_REQUIRED",
            message,
            hint: None,
        },
        PublishError::PrivateRepoNeedsPro => MappedError {
            numeric: INTERNAL_ERROR,
            code: "PRIVATE_REPO_NEEDS_PRO",
            message,
            hint: None,
        },
        PublishError::RuntimeNotReady => MappedError {
            numeric: INTERNAL_ERROR,
            code: "RUNTIME_NOT_READY",
            message,
            hint: None,
        },
        PublishError::BuildFailed { tail } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "BUILD_FAILED",
            message,
            hint: Some(tail),
        },
        PublishError::BuildOutputMissing { expected } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "BUILD_OUTPUT_MISSING",
            message,
            hint: Some(expected),
        },
        PublishError::PushFailed { reason } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "PUSH_FAILED",
            message,
            hint: Some(reason),
        },
        PublishError::PagesApiFailed { status, body } => MappedError {
            numeric: INTERNAL_ERROR,
            code: "PAGES_API_FAILED",
            message,
            hint: Some(format!("HTTP {status}: {body}")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn data_code(err: &McpError) -> String {
        err.data
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|o| o.get("code"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    fn data_hint(err: &McpError) -> Option<String> {
        err.data
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|o| o.get("hint"))
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    #[test]
    fn project_not_found_uses_specific_code() {
        let err = into_mcp(AppError::NotFound {
            entity: "project".into(),
            id: "blog".into(),
        });
        assert_eq!(err.code, INVALID_PARAMS);
        assert_eq!(data_code(&err), "PROJECT_NOT_FOUND");
        assert_eq!(data_hint(&err).as_deref(), Some("blog"));
    }

    #[test]
    fn generic_not_found_uses_generic_code() {
        let err = into_mcp(AppError::NotFound {
            entity: "tunnel".into(),
            id: "abc".into(),
        });
        assert_eq!(err.code, INVALID_PARAMS);
        assert_eq!(data_code(&err), "NOT_FOUND");
        assert_eq!(data_hint(&err).as_deref(), Some("tunnel: abc"));
    }

    #[test]
    fn invalid_input_maps_to_invalid_params() {
        let err = into_mcp(AppError::InvalidInput {
            field: "slug".into(),
            reason: "empty".into(),
        });
        assert_eq!(err.code, INVALID_PARAMS);
        assert_eq!(data_code(&err), "INVALID_INPUT");
        assert_eq!(data_hint(&err).as_deref(), Some("slug: empty"));
    }

    #[test]
    fn github_auth_required_includes_actionable_hint() {
        let err = into_mcp(AppError::AuthRequired {
            provider: "github".into(),
        });
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(data_code(&err), "NO_GITHUB_AUTH");
        assert!(data_hint(&err).expect("hint").contains("Opnble"));
    }

    #[test]
    fn vm_not_running_carries_static_hint() {
        let err = into_mcp(AppError::VmNotRunning);
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(data_code(&err), "VM_OFFLINE");
        assert!(data_hint(&err).expect("hint").contains("Start any project"));
    }

    #[test]
    fn publish_build_failed_passes_tail_as_hint() {
        let err = into_mcp(AppError::Publish(PublishError::BuildFailed {
            tail: "npm ERR! exit 1".into(),
        }));
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(data_code(&err), "BUILD_FAILED");
        assert_eq!(data_hint(&err).as_deref(), Some("npm ERR! exit 1"));
    }

    #[test]
    fn publish_pages_api_failed_formats_status_body() {
        let err = into_mcp(AppError::Publish(PublishError::PagesApiFailed {
            status: 404,
            body: "not found".into(),
        }));
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(data_code(&err), "PAGES_API_FAILED");
        assert_eq!(data_hint(&err).as_deref(), Some("HTTP 404: not found"),);
    }

    #[test]
    fn internal_carries_reason_as_message_and_hint() {
        let err = into_mcp(AppError::Internal {
            reason: "boom".into(),
        });
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(data_code(&err), "INTERNAL");
        assert_eq!(err.message, "boom");
        assert_eq!(data_hint(&err).as_deref(), Some("boom"));
    }

    #[test]
    fn no_github_auth_matches_auth_required_shape() {
        let direct = no_github_auth();
        let mapped = into_mcp(AppError::AuthRequired {
            provider: "github".into(),
        });
        assert_eq!(direct.code, mapped.code);
        assert_eq!(data_code(&direct), data_code(&mapped));
        assert_eq!(data_hint(&direct), data_hint(&mapped));
        assert_eq!(direct.message, mapped.message);
    }

    #[test]
    fn no_remote_helper_uses_invalid_params_with_create_repo_hint() {
        let err = no_remote("blog");
        assert_eq!(err.code, INVALID_PARAMS);
        assert_eq!(data_code(&err), "NO_REMOTE");
        assert!(err.message.contains("blog"));
        assert!(data_hint(&err)
            .expect("hint")
            .contains("createRepoIfMissing"));
    }

    #[test]
    fn push_failed_helper_threads_reason_into_hint() {
        let err = push_failed("blog", "non-fast-forward");
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(data_code(&err), "PUSH_FAILED");
        assert!(err.message.contains("blog"));
        assert_eq!(data_hint(&err).as_deref(), Some("non-fast-forward"));
    }

    #[test]
    fn publish_timeout_helper_carries_static_hint() {
        let err = publish_timeout("blog", 600);
        assert_eq!(err.code, INTERNAL_ERROR);
        assert_eq!(data_code(&err), "PUBLISH_TIMEOUT");
        assert!(err.message.contains("600s"));
        assert!(data_hint(&err).expect("hint").contains("10-minute"));
    }
}
