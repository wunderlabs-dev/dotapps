//! Bearer-token auth for the local MCP listener.
//!
//! The listener binds to loopback only, but on a shared dev machine that is
//! still not enough: any local process can hit `http://127.0.0.1:47821/mcp`.
//! A 256-bit token stored in `~/.opnble/mcp.json` (mode 0600 on Unix) gates
//! every request via an axum middleware. The editor reads the same file to
//! configure its MCP client.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::constants::paths;
use crate::error::AppError;
use crate::mcp::config;

/// Filename inside `~/.opnble/` that stores `{ url, token }`.
const CONFIG_FILE: &str = "mcp.json";

/// Number of random bytes that back the bearer token. 32 bytes = 256 bits =
/// 64 hex chars. Brute-forcing a single token in the loopback-only attack
/// model is infeasible, and rotating only requires rewriting the file.
const TOKEN_BYTES: usize = 32;

/// On-disk shape of `~/.opnble/mcp.json`.
///
/// `url` is informational: it mirrors `config::mcp_url()` so the editor's
/// config writer can read both values from one place. The authoritative URL
/// stays in code; this struct is the data exchange shape.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
struct McpFile {
    url: String,
    token: String,
}

/// Test seam: a token file rooted at an arbitrary directory.
///
/// Public API ([`current_token`], [`rotate_token`], [`config_path`]) wraps
/// this with `paths::base_dir()` so production always reads `~/.opnble/`.
/// Tests construct with a `TempDir` instead.
struct TokenFile {
    path: PathBuf,
}

impl TokenFile {
    fn at_base(base: &Path) -> Self {
        Self {
            path: base.join(CONFIG_FILE),
        }
    }

    /// Read the token off disk, generating + persisting on first call.
    fn current(&self) -> Result<String, AppError> {
        match std::fs::read_to_string(&self.path) {
            Ok(raw) => {
                let parsed: McpFile = serde_json::from_str(&raw)?;
                if is_valid_token(&parsed.token) {
                    Ok(parsed.token)
                } else {
                    tracing::warn!(
                        "rewriting mcp.json: stored token is not 64 hex chars (was {} chars)",
                        parsed.token.len()
                    );
                    self.rotate()
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.rotate(),
            Err(e) => Err(e.into()),
        }
    }

    /// Generate a fresh token and overwrite the file. Always rewrites.
    fn rotate(&self) -> Result<String, AppError> {
        let token = generate_token()?;
        let file = McpFile {
            url: config::mcp_url(),
            token: token.clone(),
        };
        write_atomically(&self.path, &file)?;
        Ok(token)
    }
}

/// Absolute path to `~/.opnble/mcp.json`.
#[allow(
    dead_code,
    reason = "Phase 1 surface; settings panel exposes this in Phase 8"
)]
pub fn config_path() -> Result<PathBuf, AppError> {
    Ok(TokenFile::at_base(&paths::base_dir()?).path)
}

/// Load the current bearer token, generating + persisting one on first call.
///
/// Idempotent: a follow-up call returns the same token as long as the file
/// is intact.
pub fn current_token() -> Result<String, AppError> {
    TokenFile::at_base(&paths::base_dir()?).current()
}

/// Generate a new token, overwrite the config file on disk, AND update the
/// running listener's [`SharedToken`] so existing connections see the new
/// value. Returns the new token so the caller can sweep per-project
/// `.cursor/mcp.json` files.
///
/// Both updates happen under the same `SharedToken` write lock: callers
/// observe either the old token everywhere or the new token everywhere,
/// never a state where one source disagrees with the other.
///
/// Old-token revocation is the security contract: after this returns, the
/// old token is no longer accepted by `bearer_auth_layer`, so an agent that
/// scraped it once cannot keep using it.
pub async fn rotate_token(shared: &SharedToken) -> Result<String, AppError> {
    let mut guard = shared.write().await;
    let new_token = TokenFile::at_base(&paths::base_dir()?).rotate()?;
    (*guard).clone_from(&new_token);
    Ok(new_token)
}

/// Random 256-bit token, lower-case hex encoded.
fn generate_token() -> Result<String, AppError> {
    let mut buf = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut buf).map_err(|e| AppError::Internal {
        reason: format!("cannot read OS rng for mcp token: {e}"),
    })?;
    Ok(hex_encode(&buf))
}

/// Lower-case hex encoding without pulling in the `hex` crate for 32 bytes.
fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Predicate matching the output of [`generate_token`]: 64 lower-case hex
/// chars. Rejects empty strings, uppercase, embedded whitespace, etc.
fn is_valid_token(token: &str) -> bool {
    token.len() == TOKEN_BYTES * 2
        && token
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// Write `mcp.json` so a partially-written file never replaces a good one,
/// and so the result is mode 0600 on Unix systems.
fn write_atomically(path: &Path, file: &McpFile) -> Result<(), AppError> {
    let parent = path.parent().ok_or_else(|| AppError::StorageFailed {
        reason: format!("mcp config path has no parent: {}", path.display()),
    })?;
    std::fs::create_dir_all(parent)?;

    let tmp = path.with_extension("json.tmp");
    {
        let mut handle = std::fs::File::create(&tmp)?;
        let bytes = serde_json::to_vec_pretty(file)?;
        handle.write_all(&bytes)?;
        handle.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Storage type passed to [`bearer_auth_layer`] via `from_fn_with_state`.
///
/// Wrapping the token in an `Arc<RwLock<_>>` lets a future "rotate token"
/// command propagate to in-flight middleware invocations without rebinding
/// the TCP listener.
pub type SharedToken = Arc<RwLock<String>>;

/// axum middleware: 401 unless `Authorization: Bearer <token>` matches the
/// current token. Used via
/// `axum::middleware::from_fn_with_state(token, bearer_auth_layer)`.
pub async fn bearer_auth_layer(
    State(token): State<SharedToken>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected = token.read().await.clone();
    if header_matches(request.headers(), &expected) {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Compare the `Authorization` header against the expected token in length-
/// independent constant time. Localhost-only deployment makes a timing oracle
/// unlikely, but constant-time is essentially free here so do it anyway.
fn header_matches(headers: &HeaderMap, expected: &str) -> bool {
    let Some(provided) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|raw| raw.strip_prefix("Bearer "))
    else {
        return false;
    };
    constant_time_eq(provided.as_bytes(), expected.as_bytes())
}

/// Pure-Rust constant-time byte slice compare.
///
/// Volatile reads are not used here, but the loop has no early exit and
/// `diff` accumulates over every pair. Equal lengths are required so
/// different-length tokens still take a predictable amount of time.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use tempfile::TempDir;
    use tower::ServiceExt;

    #[test]
    fn token_is_64_hex_chars() {
        let token = generate_token().expect("rng");
        assert_eq!(token.len(), 64);
        assert!(is_valid_token(&token));
    }

    #[test]
    fn current_token_creates_file_on_first_call() {
        let dir = TempDir::new().expect("tempdir");
        let file = TokenFile::at_base(dir.path());
        assert!(!file.path.exists());
        let token = file.current().expect("token");
        assert!(file.path.exists());
        assert!(is_valid_token(&token));
    }

    #[test]
    fn current_token_is_idempotent() {
        let dir = TempDir::new().expect("tempdir");
        let file = TokenFile::at_base(dir.path());
        let first = file.current().expect("first");
        let second = file.current().expect("second");
        assert_eq!(first, second);
    }

    #[test]
    fn rotate_token_changes_value() {
        let dir = TempDir::new().expect("tempdir");
        let file = TokenFile::at_base(dir.path());
        let first = file.current().expect("first");
        let second = file.rotate().expect("rotate");
        assert_ne!(first, second);
        let third = file.current().expect("third");
        assert_eq!(second, third);
    }

    #[cfg(unix)]
    #[test]
    fn config_file_mode_is_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().expect("tempdir");
        let file = TokenFile::at_base(dir.path());
        let _ = file.current().expect("token");
        let mode = std::fs::metadata(&file.path)
            .expect("meta")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "got {mode:o}");
    }

    #[test]
    fn current_token_rewrites_on_invalid_existing_file() {
        let dir = TempDir::new().expect("tempdir");
        let file = TokenFile::at_base(dir.path());
        std::fs::write(
            &file.path,
            serde_json::to_string(&McpFile {
                url: config::mcp_url(),
                token: "not-hex".into(),
            })
            .expect("ser"),
        )
        .expect("write");
        let token = file.current().expect("token");
        assert!(is_valid_token(&token));
        assert_ne!(token, "not-hex");
    }

    fn router_with_token(token: &str) -> Router {
        let shared: SharedToken = Arc::new(RwLock::new(token.to_string()));
        Router::new()
            .route("/ping", get(|| async { "pong" }))
            .layer(axum::middleware::from_fn_with_state(
                shared,
                bearer_auth_layer,
            ))
    }

    #[tokio::test]
    async fn middleware_accepts_correct_token() {
        let response = router_with_token("secret-token")
            .oneshot(
                axum::http::Request::builder()
                    .uri("/ping")
                    .header(axum::http::header::AUTHORIZATION, "Bearer secret-token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_rejects_missing_header() {
        let response = router_with_token("secret-token")
            .oneshot(
                axum::http::Request::builder()
                    .uri("/ping")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn middleware_rejects_wrong_token() {
        let response = router_with_token("secret-token")
            .oneshot(
                axum::http::Request::builder()
                    .uri("/ping")
                    .header(axum::http::header::AUTHORIZATION, "Bearer nope")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn middleware_rejects_non_bearer_scheme() {
        let response = router_with_token("secret-token")
            .oneshot(
                axum::http::Request::builder()
                    .uri("/ping")
                    .header(axum::http::header::AUTHORIZATION, "Basic secret-token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn constant_time_eq_handles_unequal_lengths() {
        assert!(!constant_time_eq(b"a", b"ab"));
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
    }

    /// Build a router that shares its `SharedToken` with the test, so the
    /// test can simulate `mcp_rotate_token` writing a new value into the
    /// same lock the listener's middleware reads from.
    fn router_with_shared(shared: SharedToken) -> Router {
        Router::new()
            .route("/ping", get(|| async { "pong" }))
            .layer(axum::middleware::from_fn_with_state(
                shared,
                bearer_auth_layer,
            ))
    }

    async fn ping(router: Router, token: &str) -> StatusCode {
        let response = router
            .oneshot(
                axum::http::Request::builder()
                    .uri("/ping")
                    .header(axum::http::header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        response.status()
    }

    /// Regression for the rotate-token bug fixed in this commit: writing a
    /// new token to the same `SharedToken` that the listener's middleware
    /// holds must invalidate the old token AND start accepting the new one
    /// without rebinding the listener.
    #[tokio::test]
    async fn rotation_via_shared_token_invalidates_old_and_accepts_new() {
        let shared: SharedToken = Arc::new(RwLock::new("old-token".to_string()));
        let router = router_with_shared(Arc::clone(&shared));

        // Sanity: old token works.
        assert_eq!(ping(router.clone(), "old-token").await, StatusCode::OK);

        // Simulate `mcp_rotate_token` updating the live SharedToken.
        *shared.write().await = "new-token".to_string();

        assert_eq!(
            ping(router.clone(), "old-token").await,
            StatusCode::UNAUTHORIZED,
            "old token must stop working immediately after rotation"
        );
        assert_eq!(
            ping(router, "new-token").await,
            StatusCode::OK,
            "new token must work without rebinding the listener"
        );
    }
}
