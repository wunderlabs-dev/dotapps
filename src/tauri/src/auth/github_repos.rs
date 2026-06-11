//! GitHub repos REST API client.
//!
//! Mirrors `publish::github_pages::GitHubPagesClient`: a focused reqwest
//! client that handles `POST /user/repos` (create) and
//! `GET /repos/{owner}/{name}` (existence check). Used by the `github_push`
//! MCP tool's `create_repo_if_missing` flow.
//!
//! Errors come back as `AppError`. `POST /user/repos` returning 422 with a
//! `name already exists` body is mapped to `AppError::AlreadyExists` so the
//! caller can surface a structured "name taken" error; everything else
//! becomes `AppError::Internal` since the caller cannot do anything
//! actionable with the underlying HTTP status. See `mcp::errors` for the
//! caller-facing mapping.

use crate::error::AppError;

const GITHUB_API_BASE: &str = "https://api.github.com";

/// REST client for the small slice of `/user/repos` and `/repos/{owner}/{name}`
/// the MCP push flow needs.
pub struct GitHubReposClient {
    http: reqwest::Client,
    api_base: String,
}

/// Subset of GitHub's repo response we care about after `create_user_repo`.
///
/// Phase 6's `github_push` only consumes `clone_url`. `html_url` and
/// `default_branch` are kept for future MCP surfaces (an "open in browser"
/// hint, default-branch resolution for fresh repos) and so the type stays
/// faithful to the GitHub response.
#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "html_url + default_branch are Phase 6 surface kept faithful to the GitHub response; Phase 7+ tools (open-in-browser, fresh-repo branch detection) will read them"
)]
pub struct CreatedRepo {
    pub clone_url: String,
    pub html_url: String,
    pub default_branch: String,
}

impl GitHubReposClient {
    pub fn new() -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("opnble-desktop")
            .build()?;
        Ok(Self {
            http,
            api_base: GITHUB_API_BASE.to_string(),
        })
    }

    #[cfg(test)]
    fn with_api_base(base: &str) -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .user_agent("opnble-desktop")
            .build()?;
        Ok(Self {
            http,
            api_base: base.to_string(),
        })
    }

    /// `POST /user/repos`. Creates a new repository on the authenticated
    /// user's account and returns its clone URL, HTML URL, and default
    /// branch.
    ///
    /// `private = true` is the safe default for the MCP push flow: a freshly
    /// created repo with no contents leaks nothing if the push fails. The
    /// caller can flip the visibility later via `PushVisibility::Public`.
    pub async fn create_user_repo(
        &self,
        token: &str,
        name: &str,
        private: bool,
    ) -> Result<CreatedRepo, AppError> {
        let url = format!("{}/user/repos", self.api_base);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({
                "name": name,
                "private": private,
                "auto_init": false,
            }))
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            // 422 with "name already exists" is the only outcome the caller
            // can react to programmatically: surface it as a structured
            // AlreadyExists. Everything else (401, 403, 5xx) is opaque.
            if status == reqwest::StatusCode::UNPROCESSABLE_ENTITY
                && body.contains("name already exists")
            {
                return Err(AppError::AlreadyExists {
                    entity: "github_repo".into(),
                    id: name.into(),
                });
            }
            return Err(AppError::Internal {
                reason: format!("create_user_repo failed: HTTP {status}: {body}"),
            });
        }

        let v: serde_json::Value = serde_json::from_str(&body)?;

        let clone_url = v
            .get("clone_url")
            .and_then(serde_json::Value::as_str)
            .map(String::from)
            .ok_or_else(|| AppError::Internal {
                reason: "missing clone_url in create_user_repo response".into(),
            })?;
        let html_url = v
            .get("html_url")
            .and_then(serde_json::Value::as_str)
            .map(String::from)
            .ok_or_else(|| AppError::Internal {
                reason: "missing html_url in create_user_repo response".into(),
            })?;
        let default_branch = v
            .get("default_branch")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("main")
            .to_string();

        Ok(CreatedRepo {
            clone_url,
            html_url,
            default_branch,
        })
    }

    /// `GET /repos/{owner}/{name}`. Returns the same [`CreatedRepo`] shape
    /// as [`Self::create_user_repo`] on 200, `None` on 404, error on any
    /// other status (a token that cannot read its own repos is
    /// misconfigured; the caller cannot recover by retrying).
    ///
    /// Used by `github_push`'s recovery path: when `create_user_repo`
    /// returns `AlreadyExists`, we still need the clone URL to set up the
    /// local `origin`, and this is the canonical way to fetch it.
    pub async fn fetch_user_repo(
        &self,
        token: &str,
        owner: &str,
        name: &str,
    ) -> Result<Option<CreatedRepo>, AppError> {
        let url = format!("{}/repos/{owner}/{name}", self.api_base);
        let resp = self
            .http
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Internal {
                reason: format!("fetch_user_repo failed: HTTP {status}: {body}"),
            });
        }

        let v: serde_json::Value = resp.json().await?;
        let clone_url = v
            .get("clone_url")
            .and_then(serde_json::Value::as_str)
            .map(String::from)
            .ok_or_else(|| AppError::Internal {
                reason: "missing clone_url in fetch_user_repo response".into(),
            })?;
        let html_url = v
            .get("html_url")
            .and_then(serde_json::Value::as_str)
            .map(String::from)
            .ok_or_else(|| AppError::Internal {
                reason: "missing html_url in fetch_user_repo response".into(),
            })?;
        let default_branch = v
            .get("default_branch")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("main")
            .to_string();

        Ok(Some(CreatedRepo {
            clone_url,
            html_url,
            default_branch,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_user_repo_201_returns_clone_url() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/user/repos")
            .match_header("Authorization", "Bearer test_token")
            .match_header("Accept", "application/vnd.github+json")
            .with_status(201)
            .with_body(
                r#"{
                "clone_url": "https://github.com/me/blog.git",
                "html_url": "https://github.com/me/blog",
                "default_branch": "main"
            }"#,
            )
            .create_async()
            .await;

        let client = GitHubReposClient::with_api_base(&server.url()).expect("build client");
        let created = client
            .create_user_repo("test_token", "blog", true)
            .await
            .expect("create succeeds");
        assert_eq!(created.clone_url, "https://github.com/me/blog.git");
        assert_eq!(created.html_url, "https://github.com/me/blog");
        assert_eq!(created.default_branch, "main");
    }

    #[tokio::test]
    async fn create_user_repo_falls_back_to_main_when_default_branch_missing() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/user/repos")
            .with_status(201)
            .with_body(
                r#"{
                "clone_url": "https://github.com/me/blog.git",
                "html_url": "https://github.com/me/blog"
            }"#,
            )
            .create_async()
            .await;

        let client = GitHubReposClient::with_api_base(&server.url()).expect("build client");
        let created = client
            .create_user_repo("t", "blog", true)
            .await
            .expect("create succeeds");
        assert_eq!(created.default_branch, "main");
    }

    #[tokio::test]
    async fn create_user_repo_422_name_taken_maps_to_already_exists() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/user/repos")
            .with_status(422)
            .with_body(
                r#"{
                "message": "Validation Failed",
                "errors": [{
                    "resource": "Repository",
                    "code": "custom",
                    "field": "name",
                    "message": "name already exists on this account"
                }]
            }"#,
            )
            .create_async()
            .await;

        let client = GitHubReposClient::with_api_base(&server.url()).expect("build client");
        let err = client
            .create_user_repo("t", "blog", true)
            .await
            .expect_err("should map to AlreadyExists");
        assert!(
            matches!(err, AppError::AlreadyExists { ref entity, ref id }
                if entity == "github_repo" && id == "blog"),
            "expected AlreadyExists, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn create_user_repo_422_other_validation_falls_through_to_internal() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/user/repos")
            .with_status(422)
            .with_body(
                r#"{
                "message": "Validation Failed",
                "errors": [{"field": "name", "code": "invalid"}]
            }"#,
            )
            .create_async()
            .await;

        let client = GitHubReposClient::with_api_base(&server.url()).expect("build client");
        let err = client
            .create_user_repo("t", "@bad", true)
            .await
            .expect_err("should error");
        assert!(
            matches!(err, AppError::Internal { .. }),
            "expected Internal, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn create_user_repo_401_unauthorized_maps_to_internal() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/user/repos")
            .with_status(401)
            .with_body(r#"{"message": "Bad credentials"}"#)
            .create_async()
            .await;

        let client = GitHubReposClient::with_api_base(&server.url()).expect("build client");
        let err = client
            .create_user_repo("bad_token", "blog", true)
            .await
            .expect_err("should error");
        assert!(
            matches!(err, AppError::Internal { ref reason } if reason.contains("401")),
            "expected Internal mentioning 401, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn create_user_repo_missing_clone_url_errors() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/user/repos")
            .with_status(201)
            .with_body(r#"{"html_url": "https://github.com/me/blog"}"#)
            .create_async()
            .await;

        let client = GitHubReposClient::with_api_base(&server.url()).expect("build client");
        let err = client
            .create_user_repo("t", "blog", true)
            .await
            .expect_err("should error");
        assert!(
            matches!(err, AppError::Internal { ref reason } if reason.contains("clone_url")),
            "expected Internal mentioning clone_url, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn fetch_user_repo_200_returns_clone_url() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/repos/me/blog")
            .match_header("Authorization", "Bearer test_token")
            .with_status(200)
            .with_body(
                r#"{
                    "clone_url": "https://github.com/me/blog.git",
                    "html_url": "https://github.com/me/blog",
                    "default_branch": "main"
                }"#,
            )
            .create_async()
            .await;

        let client = GitHubReposClient::with_api_base(&server.url()).expect("build client");
        let existing = client
            .fetch_user_repo("test_token", "me", "blog")
            .await
            .expect("fetch_user_repo succeeds")
            .expect("repo present");
        assert_eq!(existing.clone_url, "https://github.com/me/blog.git");
        assert_eq!(existing.default_branch, "main");
    }

    #[tokio::test]
    async fn fetch_user_repo_404_returns_none() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/repos/me/missing")
            .with_status(404)
            .with_body(r#"{"message": "Not Found"}"#)
            .create_async()
            .await;

        let client = GitHubReposClient::with_api_base(&server.url()).expect("build client");
        let result = client
            .fetch_user_repo("t", "me", "missing")
            .await
            .expect("fetch_user_repo succeeds on 404");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_user_repo_500_errors() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/repos/me/blog")
            .with_status(500)
            .with_body(r#"{"message": "Internal Server Error"}"#)
            .create_async()
            .await;

        let client = GitHubReposClient::with_api_base(&server.url()).expect("build client");
        let err = client
            .fetch_user_repo("t", "me", "blog")
            .await
            .expect_err("should error on 500");
        assert!(
            matches!(err, AppError::Internal { ref reason } if reason.contains("500")),
            "expected Internal mentioning 500, got: {err:?}"
        );
    }
}
