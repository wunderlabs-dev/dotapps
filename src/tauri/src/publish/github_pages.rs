//! GitHub Pages REST API client
//!
//! Enables, disables, and queries GitHub Pages configuration.
//! Uses the same `reqwest` + bearer token pattern as `tunnel::api`.

use crate::error::AppError;
use crate::publish::types::RepoIdentity;
use crate::publish::PublishError;

const GITHUB_API_BASE: &str = "https://api.github.com";

/// Wraps `reqwest::Error` for publish operations so the `From` impl routes
/// to `AppError::Publish(PagesApiFailed)` (with error kind classification and
/// per-operation context), instead of the global `From<reqwest::Error>`
/// which maps to `AppError::Internal`.
struct PublishHttpError {
    reason: &'static str,
    source: reqwest::Error,
}

impl PublishHttpError {
    /// Builds a closure for `map_err` that captures the operation context.
    fn wrap(reason: &'static str) -> impl FnOnce(reqwest::Error) -> Self {
        move |source| Self { reason, source }
    }
}

impl From<PublishHttpError> for AppError {
    fn from(e: PublishHttpError) -> Self {
        // Status code: real HTTP code when one is present, 0 as a sentinel
        // when reqwest never reached the server (timeout, dns, tls, etc.).
        let status = e.source.status().map_or(0, |s| s.as_u16());
        let body = if status == 0 {
            let kind = if e.source.is_builder() {
                "client"
            } else if e.source.is_timeout() {
                "timeout"
            } else if e.source.is_connect() {
                "connection"
            } else if e.source.is_decode() {
                "decode"
            } else {
                "request"
            };
            let reason = e.reason;
            let source = &e.source;
            format!("{reason} ({kind}): {source}")
        } else {
            let reason = e.reason;
            let source = &e.source;
            format!("{reason}: {source}")
        };
        Self::Publish(PublishError::PagesApiFailed { status, body })
    }
}

/// GitHub Pages API client
pub struct GitHubPagesClient {
    http: reqwest::Client,
}

impl GitHubPagesClient {
    pub fn new() -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("opnble-desktop")
            .build()
            .map_err(PublishHttpError::wrap("cannot build http client"))?;
        Ok(Self { http })
    }

    /// Check if the repository is private (Pages requires Pro for private repos).
    pub async fn is_repo_private(
        &self,
        identity: &RepoIdentity,
        token: &str,
    ) -> Result<bool, AppError> {
        let url = format!(
            "{GITHUB_API_BASE}/repos/{}/{}",
            identity.owner, identity.repo
        );
        let resp = self
            .http
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(PublishHttpError::wrap("cannot check repository visibility"))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(PublishError::PagesApiFailed { status, body }.into());
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(PublishHttpError::wrap("cannot parse repo response"))?;

        Ok(body
            .get("private")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false))
    }

    /// Check if GitHub Pages is already enabled on the repo.
    /// Returns the current Pages URL if enabled, None if not.
    pub async fn pages_status(
        &self,
        identity: &RepoIdentity,
        token: &str,
    ) -> Result<Option<String>, AppError> {
        let url = format!(
            "{GITHUB_API_BASE}/repos/{}/{}/pages",
            identity.owner, identity.repo
        );
        let resp = self
            .http
            .get(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(PublishHttpError::wrap("cannot check pages status"))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(PublishError::PagesApiFailed { status, body }.into());
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(PublishHttpError::wrap("cannot parse pages response"))?;

        let url = body
            .get("html_url")
            .and_then(|v| v.as_str())
            .map(String::from);

        Ok(url)
    }

    /// Enable GitHub Pages on the repo, serving from the gh-pages branch root.
    ///
    /// Checks if Pages is already enabled first. If not, attempts to create via
    /// the API. On 403 (token lacks Pages permission), retries `pages_status`
    /// since GitHub often auto-enables Pages when a gh-pages branch is pushed.
    pub async fn enable_pages(
        &self,
        identity: &RepoIdentity,
        token: &str,
    ) -> Result<String, AppError> {
        // Already enabled: gh-pages content was just pushed, nothing else to do
        if let Some(url) = self.pages_status(identity, token).await? {
            return Ok(url);
        }

        // Try to create Pages site via API
        match self.create_pages_site(identity, token).await {
            Ok(url) => Ok(url),
            Err(e) => {
                // On 403, GitHub may auto-enable Pages from the gh-pages push.
                // Wait briefly and check again before failing.
                tracing::warn!("Pages API creation failed, checking if auto-enabled: {e}");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                if let Some(url) = self.pages_status(identity, token).await? {
                    return Ok(url);
                }
                Err(e)
            }
        }
    }

    /// POST to create a GitHub Pages site. Returns the Pages URL on success.
    async fn create_pages_site(
        &self,
        identity: &RepoIdentity,
        token: &str,
    ) -> Result<String, AppError> {
        let url = format!(
            "{GITHUB_API_BASE}/repos/{}/{}/pages",
            identity.owner, identity.repo
        );
        let resp = self
            .http
            .post(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({
                "source": {
                    "branch": "gh-pages",
                    "path": "/"
                }
            }))
            .send()
            .await
            .map_err(PublishHttpError::wrap("cannot enable GitHub Pages"))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(PublishError::PagesApiFailed { status, body }.into());
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(PublishHttpError::wrap("cannot parse enable pages response"))?;

        let pages_url = body
            .get("html_url")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| {
                AppError::Publish(PublishError::PagesApiFailed {
                    status: 200,
                    body: "missing html_url in GitHub Pages API response".into(),
                })
            })?;

        Ok(pages_url)
    }

    /// Poll GitHub Pages deployment until status is "built" or timeout.
    ///
    /// Returns the Pages URL once the deployment completes. Polls every 5 seconds
    /// for up to 2 minutes.
    pub async fn wait_for_deployment(
        &self,
        identity: &RepoIdentity,
        token: &str,
        pages_url: &str,
    ) -> Result<String, AppError> {
        let url = format!(
            "{GITHUB_API_BASE}/repos/{}/{}/pages",
            identity.owner, identity.repo
        );

        let poll_interval = std::time::Duration::from_secs(5);
        let max_attempts: u32 = 24; // 24 * 5s = 2 minutes

        for attempt in 0..max_attempts {
            let resp = self
                .http
                .get(&url)
                .bearer_auth(token)
                .header("Accept", "application/vnd.github+json")
                .send()
                .await
                .map_err(PublishHttpError::wrap("cannot check deployment status"))?;

            if resp.status().is_success() {
                let body: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(PublishHttpError::wrap("cannot parse deployment response"))?;

                let status = body
                    .get("status")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown");

                tracing::debug!("pages deployment poll {attempt}/{max_attempts}: status={status}");

                if status == "built" {
                    return Ok(pages_url.to_string());
                }
            }

            tokio::time::sleep(poll_interval).await;
        }

        // Timed out but Pages is enabled; return URL anyway (deploy may still be in progress)
        tracing::warn!(
            "pages deployment did not reach 'built' after {max_attempts} polls, returning URL anyway"
        );
        Ok(pages_url.to_string())
    }

    /// Disable GitHub Pages on the repo.
    pub async fn disable_pages(
        &self,
        identity: &RepoIdentity,
        token: &str,
    ) -> Result<(), AppError> {
        let url = format!(
            "{GITHUB_API_BASE}/repos/{}/{}/pages",
            identity.owner, identity.repo
        );
        let resp = self
            .http
            .delete(&url)
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(PublishHttpError::wrap("cannot disable GitHub Pages"))?;

        // 404 is fine (already disabled)
        if !resp.status().is_success() && resp.status() != reqwest::StatusCode::NOT_FOUND {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(PublishError::PagesApiFailed { status, body }.into());
        }

        Ok(())
    }
}
