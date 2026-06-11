//! Authentication service
//!
//! Handles token management and OAuth flows for Git providers.

use std::sync::Arc;

use tokio::sync::Mutex as TokioMutex;

use super::store::TokenStore;
use super::types::{AccessTokenResponse, DeviceCodeResponse, GitHubRepo, GitHubUser};
use crate::error::AppError;

/// GitHub OAuth App Client ID
/// Users can create their own at: <https://github.com/settings/applications/new>
const GITHUB_CLIENT_ID: &str = "Iv23liXGiAg0d4h12mih";

const GITHUB_API_BASE: &str = "https://api.github.com";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_PROVIDER: &str = "github";

/// Authentication service for managing tokens and OAuth flows
pub struct Authenticator {
    token_store: Arc<dyn TokenStore>,
    http: reqwest::Client,
    github_api_base: String,
    refresh_lock: TokioMutex<()>,
}

impl Authenticator {
    /// Create a new Authenticator with the given token store
    pub fn new(token_store: Arc<dyn TokenStore>) -> Self {
        Self {
            token_store,
            http: reqwest::Client::new(),
            github_api_base: GITHUB_API_BASE.to_string(),
            refresh_lock: TokioMutex::new(()),
        }
    }

    #[cfg(test)]
    fn with_github_api_base(token_store: Arc<dyn TokenStore>, base: &str) -> Self {
        Self {
            token_store,
            http: reqwest::Client::new(),
            github_api_base: base.to_string(),
            refresh_lock: TokioMutex::new(()),
        }
    }

    fn token_url(&self) -> String {
        if self.github_api_base == GITHUB_API_BASE {
            GITHUB_TOKEN_URL.to_string()
        } else {
            format!("{}/login/oauth/access_token", self.github_api_base)
        }
    }

    /// Store a token for a provider
    pub fn store_token(&self, provider: &str, token: &str) -> Result<(), AppError> {
        tracing::debug!(provider, token_len = token.len(), "store_token");
        let result = self.token_store.store(provider, token);
        match &result {
            Ok(()) => tracing::debug!(provider, "store_token: ok"),
            Err(e) => tracing::debug!(provider, error = %e, "store_token: failed"),
        }
        result
    }

    /// Get a token for a provider
    pub fn token(&self, provider: &str) -> Result<Option<String>, AppError> {
        let result = self.token_store.get(provider);
        match &result {
            Ok(Some(token)) => tracing::debug!(
                provider,
                present = true,
                token_len = token.len(),
                "token lookup"
            ),
            Ok(None) => tracing::debug!(provider, present = false, "token lookup"),
            Err(e) => tracing::debug!(provider, error = %e, "token lookup: failed"),
        }
        result
    }

    /// Delete a token and its associated refresh token for a provider
    pub fn delete_token(&self, provider: &str) -> Result<(), AppError> {
        tracing::debug!("delete_token({provider})");
        let refresh_key = format!("{provider}:refresh");
        let _ = self.token_store.delete(&refresh_key);
        self.token_store.delete(provider)
    }

    /// Determine provider from repository URL
    pub fn provider_from_url(url: &str) -> Option<&'static str> {
        match Self::extract_host(url).as_deref() {
            Some("github.com") => Some("github"),
            _ => None,
        }
    }

    /// Detect known but unsupported providers, returning a display name.
    pub fn is_unsupported_provider(url: &str) -> Option<&'static str> {
        match Self::extract_host(url).as_deref() {
            Some("gitlab.com") => Some("GitLab"),
            Some("bitbucket.org") => Some("Bitbucket"),
            _ => None,
        }
    }

    /// Validate a token with a lightweight API call.
    ///
    /// Returns `Ok(true)` when the token is valid, `Ok(false)` only when the
    /// provider explicitly rejects the token (HTTP 401). Returns `Err` for any
    /// other non-success status (rate-limit 403, server error 5xx) so callers
    /// can keep the token rather than deleting it on transient failures.
    pub async fn validate_token(&self, provider: &str, token: &str) -> Result<bool, AppError> {
        match provider {
            "github" => {
                let resp = self
                    .http
                    .get(format!("{}/user", self.github_api_base))
                    .header("Authorization", format!("Bearer {token}"))
                    .header("User-Agent", "Opnble")
                    .send()
                    .await
                    .map_err(|e| AppError::Internal {
                        reason: e.to_string(),
                    })?;

                let status = resp.status();
                if status.is_success() {
                    return Ok(true);
                }
                if status == reqwest::StatusCode::UNAUTHORIZED {
                    return Ok(false);
                }
                // Any other non-success (403 rate-limit, 5xx server error):
                // treat as transient so the caller keeps the token.
                Err(AppError::Internal {
                    reason: format!("github token validation returned {status}"),
                })
            }
            _ => Ok(false),
        }
    }

    /// Resolve a valid push token for the given repo URL.
    ///
    /// Returns `Ok(None)` for repos that don't need authentication (unknown host).
    /// Returns `Err(AuthRequired)` when the provider is recognized but no valid token exists.
    pub async fn resolve_push_token(&self, repo_url: &str) -> Result<Option<String>, AppError> {
        let Some(provider) = Self::provider_from_url(repo_url) else {
            return Ok(None);
        };
        let token = self.valid_token(provider).await?;
        if token.is_none() {
            return Err(AppError::AuthRequired {
                provider: provider.to_string(),
            });
        }
        Ok(token)
    }

    /// Get a validated token. Attempts refresh on 401 before clearing.
    pub async fn valid_token(&self, provider: &str) -> Result<Option<String>, AppError> {
        let Some(token) = self.token_store.get(provider)? else {
            return Ok(None);
        };

        match self.validate_token(provider, &token).await {
            Ok(true) => Ok(Some(token)),
            Ok(false) => {
                // Token is definitely invalid (GitHub returned 401).
                // Try to refresh before giving up.
                tracing::info!("access token for {provider} expired, attempting refresh");
                self.try_refresh(provider).await
            }
            Err(e) => {
                // Network error or timeout: keep the token (might work later).
                // Don't delete on transient failures.
                tracing::warn!("cannot validate {provider} token: {e}, keeping it");
                Ok(Some(token))
            }
        }
    }

    /// Single-flight refresh: callers serialize on `refresh_lock` only to
    /// read the refresh token; the HTTP exchange runs lock-free.
    async fn try_refresh(&self, provider: &str) -> Result<Option<String>, AppError> {
        let refresh_key = format!("{provider}:refresh");
        let refresh_token = {
            let _guard = self.refresh_lock.lock().await;
            let Some(refresh_token) = self.token_store.get(&refresh_key)? else {
                let _ = self.token_store.delete(provider);
                return Ok(None);
            };
            refresh_token
        };

        match self.exchange_refresh_token(&refresh_token).await {
            Ok(response) => {
                if let Some(ref access_token) = response.access_token {
                    self.token_store.store(provider, access_token)?;
                }
                if let Some(ref new_refresh) = response.refresh_token {
                    self.token_store.store(&refresh_key, new_refresh)?;
                }
                Ok(response.access_token)
            }
            Err(e) => {
                tracing::warn!("token refresh for {provider} failed: {e}");
                let _ = self.token_store.delete(provider);
                let _ = self.token_store.delete(&refresh_key);
                Ok(None)
            }
        }
    }

    /// Exchange a refresh token for a new access + refresh token pair.
    ///
    /// GitHub returns HTTP 200 with an `error` field in the body for token
    /// endpoint errors (not HTTP 4xx).
    async fn exchange_refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<AccessTokenResponse, AppError> {
        let response = self
            .http
            .post(self.token_url())
            .header("Accept", "application/json")
            .form(&[
                ("client_id", GITHUB_CLIENT_ID),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
            ])
            .send()
            .await
            .map_err(|e| AppError::AuthFailed {
                reason: format!("cannot refresh token: {e}"),
            })?;

        let token_response: AccessTokenResponse =
            response.json().await.map_err(|e| AppError::AuthFailed {
                reason: format!("cannot parse refresh response: {e}"),
            })?;

        // GitHub returns 200 with error body for token endpoint errors
        if let Some(ref error) = token_response.error {
            return Err(AppError::AuthFailed {
                reason: format!(
                    "token refresh failed: {error}: {}",
                    token_response
                        .error_description
                        .as_deref()
                        .unwrap_or("no details")
                ),
            });
        }

        if token_response.access_token.is_none() {
            return Err(AppError::AuthFailed {
                reason: "token refresh returned no access token".to_string(),
            });
        }

        Ok(token_response)
    }

    /// Start GitHub Device OAuth flow
    ///
    /// Returns device code information for the user to complete authorization.
    /// The user should visit `verification_uri` and enter the `user_code`.
    pub async fn github_start_oauth(
        &self,
        client_id: Option<&str>,
    ) -> Result<DeviceCodeResponse, AppError> {
        let client = &self.http;
        let cid = client_id.unwrap_or(GITHUB_CLIENT_ID);

        let response = client
            .post("https://github.com/login/device/code")
            .header("Accept", "application/json")
            .form(&[("client_id", cid), ("scope", "repo")])
            .send()
            .await?;

        Self::parse_github_response(response, "start oauth").await
    }

    /// Poll GitHub for access token during Device OAuth flow
    ///
    /// Call this repeatedly at the interval specified in `DeviceCodeResponse`
    /// until either an `access_token` or a terminal error is returned.
    /// Stores the refresh token in the keychain if present in the response.
    pub async fn github_poll_oauth(
        &self,
        device_code: &str,
        client_id: Option<&str>,
    ) -> Result<AccessTokenResponse, AppError> {
        let client = &self.http;
        let cid = client_id.unwrap_or(GITHUB_CLIENT_ID);

        let response = client
            .post(self.token_url())
            .header("Accept", "application/json")
            .form(&[
                ("client_id", cid),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?;

        let token_response: AccessTokenResponse =
            Self::parse_github_response(response, "poll oauth").await?;

        // Store refresh token if present (GitHub Apps with token expiration enabled)
        if let Some(ref refresh_token) = token_response.refresh_token {
            let refresh_key = format!("{GITHUB_PROVIDER}:refresh");
            if let Err(e) = self.token_store.store(&refresh_key, refresh_token) {
                tracing::warn!("cannot store refresh token for {GITHUB_PROVIDER}: {e}");
            }
        }

        Ok(token_response)
    }

    /// Get current GitHub user information
    pub async fn github_user(&self, token: &str) -> Result<GitHubUser, AppError> {
        let client = &self.http;

        let response = client
            .get(format!("{}/user", self.github_api_base))
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "Opnble")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;

        Self::parse_github_response(response, "fetch user").await
    }

    /// List user's GitHub repositories
    pub async fn github_list_repos(
        &self,
        token: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<GitHubRepo>, AppError> {
        let client = &self.http;

        let response = client
            .get(format!("{}/user/repos", self.github_api_base))
            .header("Authorization", format!("Bearer {token}"))
            .header("User-Agent", "Opnble")
            .header("Accept", "application/vnd.github+json")
            .query(&[
                ("sort", "updated"),
                ("per_page", &per_page.to_string()),
                ("page", &page.to_string()),
            ])
            .send()
            .await?;

        Self::parse_github_response(response, "list repos").await
    }

    fn extract_host(url: &str) -> Option<String> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return None;
        }

        // SCP-style SSH URL: git@host:owner/repo.git
        if let Some(rest) = trimmed.strip_prefix("git@") {
            let host = rest.split(':').next()?.trim();
            if host.is_empty() {
                return None;
            }
            return Some(host.to_ascii_lowercase());
        }

        // Standard URL: scheme://[userinfo@]host[:port]/path
        let (_, after_scheme) = trimmed.split_once("://")?;
        let authority = after_scheme.split('/').next()?.trim();
        if authority.is_empty() {
            return None;
        }

        let host_port = authority.rsplit('@').next().unwrap_or(authority);
        let host = if let Some(stripped) = host_port.strip_prefix('[') {
            stripped.split(']').next().unwrap_or(stripped)
        } else {
            host_port.split(':').next().unwrap_or(host_port)
        };
        let host = host.trim();
        if host.is_empty() {
            return None;
        }

        Some(host.to_ascii_lowercase())
    }

    async fn parse_github_response<T>(
        response: reqwest::Response,
        operation: &str,
    ) -> Result<T, AppError>
    where
        T: serde::de::DeserializeOwned,
    {
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::AuthFailed {
                reason: format!("cannot {operation}: github returned {status}"),
            });
        }
        response
            .json::<T>()
            .await
            .map_err(|e| AppError::AuthFailed {
                reason: format!("cannot parse {operation} response: {e}"),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock token store for testing
    #[expect(
        clippy::disallowed_types,
        reason = "test mock uses sync Mutex for simplicity"
    )]
    struct MockTokenStore {
        tokens: std::sync::Mutex<std::collections::HashMap<String, String>>,
    }

    #[expect(
        clippy::disallowed_types,
        reason = "test mock uses sync Mutex for simplicity"
    )]
    impl MockTokenStore {
        fn new() -> Self {
            Self {
                tokens: std::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }
    }

    impl TokenStore for MockTokenStore {
        fn store(&self, provider: &str, token: &str) -> Result<(), AppError> {
            self.tokens
                .lock()
                .unwrap()
                .insert(provider.to_string(), token.to_string());
            Ok(())
        }

        fn get(&self, provider: &str) -> Result<Option<String>, AppError> {
            Ok(self.tokens.lock().unwrap().get(provider).cloned())
        }

        fn delete(&self, provider: &str) -> Result<(), AppError> {
            self.tokens.lock().unwrap().remove(provider);
            Ok(())
        }
    }

    #[test]
    fn test_provider_from_url_github() {
        assert_eq!(
            Authenticator::provider_from_url("https://github.com/user/repo"),
            Some("github")
        );
        assert_eq!(
            Authenticator::provider_from_url("git@github.com:user/repo.git"),
            Some("github")
        );
        assert_eq!(
            Authenticator::provider_from_url("ssh://git@github.com:22/user/repo.git"),
            Some("github")
        );
    }

    #[test]
    fn test_provider_from_url_gitlab() {
        assert_eq!(
            Authenticator::provider_from_url("https://gitlab.com/user/repo"),
            None
        );
    }

    #[test]
    fn test_unsupported_provider_gitlab() {
        assert_eq!(
            Authenticator::is_unsupported_provider("https://gitlab.com/user/repo"),
            Some("GitLab")
        );
    }

    #[test]
    fn test_unsupported_provider_bitbucket() {
        assert_eq!(
            Authenticator::is_unsupported_provider("https://bitbucket.org/user/repo"),
            Some("Bitbucket")
        );
    }

    #[test]
    fn test_unsupported_provider_github() {
        assert_eq!(
            Authenticator::is_unsupported_provider("https://github.com/user/repo"),
            None
        );
    }

    #[test]
    fn test_provider_from_url_unknown() {
        assert_eq!(
            Authenticator::provider_from_url("https://bitbucket.org/user/repo"),
            None
        );
        assert_eq!(
            Authenticator::provider_from_url("https://example.com/repo"),
            None
        );
        assert_eq!(
            Authenticator::provider_from_url("https://github.com.evil.example/repo"),
            None
        );
        assert_eq!(
            Authenticator::provider_from_url("https://evil-github.com/repo"),
            None
        );
    }

    #[test]
    fn test_store_and_get_token() {
        let store = Arc::new(MockTokenStore::new());
        let auth = Authenticator::new(store);

        auth.store_token("github", "test_token").unwrap();
        let token = auth.token("github").unwrap();
        assert_eq!(token, Some("test_token".to_string()));
    }

    #[test]
    fn test_delete_token() {
        let store = Arc::new(MockTokenStore::new());
        let auth = Authenticator::new(store);

        auth.store_token("github", "test_token").unwrap();
        auth.delete_token("github").unwrap();
        let token = auth.token("github").unwrap();
        assert_eq!(token, None);
    }

    #[test]
    fn test_get_missing_token() {
        let store = Arc::new(MockTokenStore::new());
        let auth = Authenticator::new(store);

        let token = auth.token("nonexistent").unwrap();
        assert_eq!(token, None);
    }

    #[tokio::test]
    async fn test_validate_token_success() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/user")
            .match_header("Authorization", "Bearer valid_token")
            .with_status(200)
            .with_body(r#"{"login":"test","avatar_url":"https://example.com"}"#)
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        let auth = Authenticator::with_github_api_base(store, &server.url());
        let valid = auth.validate_token("github", "valid_token").await.unwrap();
        assert!(valid);
    }

    #[tokio::test]
    async fn test_validate_token_expired() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/user")
            .match_header("Authorization", "Bearer expired_token")
            .with_status(401)
            .with_body(r#"{"message":"Bad credentials"}"#)
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        let auth = Authenticator::with_github_api_base(store, &server.url());
        let valid = auth
            .validate_token("github", "expired_token")
            .await
            .unwrap();
        assert!(!valid);
    }

    #[tokio::test]
    async fn test_validate_token_unknown_provider() {
        let store = Arc::new(MockTokenStore::new());
        let auth = Authenticator::new(store);
        let valid = auth
            .validate_token("bitbucket", "some_token")
            .await
            .unwrap();
        assert!(!valid);
    }

    #[tokio::test]
    async fn test_validate_token_rate_limited_returns_err() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/user")
            .match_header("Authorization", "Bearer good_token")
            .with_status(403)
            .with_body(r#"{"message":"API rate limit exceeded"}"#)
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        let auth = Authenticator::with_github_api_base(store, &server.url());
        let result = auth.validate_token("github", "good_token").await;
        assert!(
            result.is_err(),
            "non-401 errors must return Err so token is kept"
        );
    }

    #[tokio::test]
    async fn test_validate_token_server_error_returns_err() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/user")
            .match_header("Authorization", "Bearer good_token")
            .with_status(500)
            .with_body(r#"{"message":"Internal Server Error"}"#)
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        let auth = Authenticator::with_github_api_base(store, &server.url());
        let result = auth.validate_token("github", "good_token").await;
        assert!(
            result.is_err(),
            "server errors must return Err so token is kept"
        );
    }

    #[tokio::test]
    async fn test_valid_token_keeps_token_on_rate_limit() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/user")
            .with_status(403)
            .with_body(r#"{"message":"API rate limit exceeded"}"#)
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        store.store("github", "good_token").unwrap();

        let auth = Authenticator::with_github_api_base(
            #[expect(clippy::as_conversions, reason = "safe trait object upcast in test")]
            {
                Arc::clone(&store) as Arc<dyn TokenStore>
            },
            &server.url(),
        );
        let result = auth.valid_token("github").await.unwrap();
        assert_eq!(
            result,
            Some("good_token".to_string()),
            "token must survive rate-limit"
        );
        assert_eq!(store.get("github").unwrap(), Some("good_token".to_string()));
    }

    #[tokio::test]
    async fn test_valid_token_clears_on_invalid() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/user")
            .with_status(401)
            .with_body(r#"{"message":"Bad credentials"}"#)
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        store.store("github", "bad_token").unwrap();

        let auth = Authenticator::with_github_api_base(
            #[expect(clippy::as_conversions, reason = "safe trait object upcast in test")]
            {
                Arc::clone(&store) as Arc<dyn TokenStore>
            },
            &server.url(),
        );
        let result = auth.valid_token("github").await.unwrap();
        assert_eq!(result, None);

        // Verify token was cleared from store
        assert_eq!(store.get("github").unwrap(), None);
    }

    #[tokio::test]
    async fn test_valid_token_returns_valid() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/user")
            .with_status(200)
            .with_body(r#"{"login":"test","avatar_url":"https://example.com"}"#)
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        store.store("github", "good_token").unwrap();

        let auth = Authenticator::with_github_api_base(
            #[expect(clippy::as_conversions, reason = "safe trait object upcast in test")]
            {
                Arc::clone(&store) as Arc<dyn TokenStore>
            },
            &server.url(),
        );
        let result = auth.valid_token("github").await.unwrap();
        assert_eq!(result, Some("good_token".to_string()));

        // Token should still be in store
        assert_eq!(store.get("github").unwrap(), Some("good_token".to_string()));
    }

    #[tokio::test]
    async fn test_valid_token_missing() {
        let store = Arc::new(MockTokenStore::new());
        let auth = Authenticator::new(store);
        let result = auth.valid_token("github").await.unwrap();
        assert_eq!(result, None);
    }

    /// Token store that always returns errors from `get()`
    struct FailingTokenStore;

    impl TokenStore for FailingTokenStore {
        fn store(&self, _provider: &str, _token: &str) -> Result<(), AppError> {
            Err(AppError::AuthFailed {
                reason: "simulated store failure".to_string(),
            })
        }

        fn get(&self, _provider: &str) -> Result<Option<String>, AppError> {
            Err(AppError::AuthFailed {
                reason: "simulated keychain access failure".to_string(),
            })
        }

        fn delete(&self, _provider: &str) -> Result<(), AppError> {
            Err(AppError::AuthFailed {
                reason: "simulated delete failure".to_string(),
            })
        }
    }

    #[tokio::test]
    async fn test_valid_token_propagates_store_error() {
        let store = Arc::new(FailingTokenStore);
        let auth = Authenticator::new(store);
        let result = auth.valid_token("github").await;
        assert!(result.is_err());
    }

    // --- Refresh token tests ---
    // Serde tests for AccessTokenResponse live in auth::types::tests

    #[tokio::test]
    async fn test_poll_oauth_stores_refresh_token() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/login/oauth/access_token")
            .with_status(200)
            .with_body(
                r#"{
                "access_token": "ghu_access",
                "token_type": "bearer",
                "scope": "",
                "refresh_token": "ghr_refresh",
                "refresh_token_expires_in": 15897600
            }"#,
            )
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        let auth = Authenticator::with_github_api_base(
            #[expect(clippy::as_conversions, reason = "safe trait object upcast in test")]
            {
                Arc::clone(&store) as Arc<dyn TokenStore>
            },
            &server.url(),
        );

        let response = auth
            .github_poll_oauth("device_code_123", None)
            .await
            .unwrap();
        assert_eq!(response.access_token, Some("ghu_access".to_string()));
        assert_eq!(
            store.get("github:refresh").unwrap(),
            Some("ghr_refresh".to_string()),
        );
    }

    #[tokio::test]
    async fn test_poll_oauth_no_refresh_token_skips_store() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/login/oauth/access_token")
            .with_status(200)
            .with_body(
                r#"{
                "access_token": "gho_legacy",
                "token_type": "bearer",
                "scope": "repo"
            }"#,
            )
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        let auth = Authenticator::with_github_api_base(
            #[expect(clippy::as_conversions, reason = "safe trait object upcast in test")]
            {
                Arc::clone(&store) as Arc<dyn TokenStore>
            },
            &server.url(),
        );

        let response = auth
            .github_poll_oauth("device_code_123", None)
            .await
            .unwrap();
        assert_eq!(response.access_token, Some("gho_legacy".to_string()));
        assert_eq!(store.get("github:refresh").unwrap(), None);
    }

    #[tokio::test]
    async fn test_exchange_refresh_token_success() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/login/oauth/access_token")
            .match_header("Accept", "application/json")
            .with_status(200)
            .with_body(
                r#"{
                "access_token": "ghu_new_access",
                "token_type": "bearer",
                "scope": "",
                "refresh_token": "ghr_new_refresh",
                "refresh_token_expires_in": 15897600
            }"#,
            )
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        let auth = Authenticator::with_github_api_base(store, &server.url());
        let response = auth
            .exchange_refresh_token("ghr_old_refresh")
            .await
            .unwrap();
        assert_eq!(response.access_token, Some("ghu_new_access".to_string()));
        assert_eq!(response.refresh_token, Some("ghr_new_refresh".to_string()));
    }

    #[tokio::test]
    async fn test_exchange_refresh_token_expired() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("POST", "/login/oauth/access_token")
            .with_status(200)
            .with_body(
                r#"{
                "error": "bad_refresh_token",
                "error_description": "The refresh token is invalid or has expired."
            }"#,
            )
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        let auth = Authenticator::with_github_api_base(store, &server.url());
        let result = auth.exchange_refresh_token("ghr_expired").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_try_refresh_success() {
        let mut server = mockito::Server::new_async().await;
        let _validate = server
            .mock("GET", "/user")
            .with_status(401)
            .with_body(r#"{"message":"Bad credentials"}"#)
            .expect_at_least(1)
            .create_async()
            .await;
        let _refresh = server
            .mock("POST", "/login/oauth/access_token")
            .with_status(200)
            .with_body(
                r#"{
                "access_token": "ghu_refreshed",
                "token_type": "bearer",
                "scope": "",
                "refresh_token": "ghr_new",
                "refresh_token_expires_in": 15897600
            }"#,
            )
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        store.store("github", "ghu_expired").unwrap();
        store.store("github:refresh", "ghr_old").unwrap();

        let auth = Authenticator::with_github_api_base(
            #[expect(clippy::as_conversions, reason = "safe trait object upcast in test")]
            {
                Arc::clone(&store) as Arc<dyn TokenStore>
            },
            &server.url(),
        );

        let result = auth.try_refresh("github").await.unwrap();
        assert_eq!(result, Some("ghu_refreshed".to_string()));
        assert_eq!(
            store.get("github").unwrap(),
            Some("ghu_refreshed".to_string())
        );
        assert_eq!(
            store.get("github:refresh").unwrap(),
            Some("ghr_new".to_string())
        );
    }

    #[tokio::test]
    async fn test_try_refresh_no_refresh_token_clears_access() {
        let store = Arc::new(MockTokenStore::new());
        store.store("github", "ghu_expired").unwrap();

        let auth = Authenticator::new(
            #[expect(clippy::as_conversions, reason = "safe trait object upcast in test")]
            {
                Arc::clone(&store) as Arc<dyn TokenStore>
            },
        );

        let result = auth.try_refresh("github").await.unwrap();
        assert_eq!(result, None);
        assert_eq!(store.get("github").unwrap(), None);
    }

    #[tokio::test]
    async fn test_try_refresh_expired_refresh_token_clears_both() {
        let mut server = mockito::Server::new_async().await;
        let _validate = server
            .mock("GET", "/user")
            .with_status(401)
            .create_async()
            .await;
        let _refresh = server
            .mock("POST", "/login/oauth/access_token")
            .with_status(200)
            .with_body(
                r#"{
                "error": "bad_refresh_token",
                "error_description": "The refresh token is invalid."
            }"#,
            )
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        store.store("github", "ghu_expired").unwrap();
        store.store("github:refresh", "ghr_also_expired").unwrap();

        let auth = Authenticator::with_github_api_base(
            #[expect(clippy::as_conversions, reason = "safe trait object upcast in test")]
            {
                Arc::clone(&store) as Arc<dyn TokenStore>
            },
            &server.url(),
        );

        let result = auth.try_refresh("github").await.unwrap();
        assert_eq!(result, None);
        assert_eq!(store.get("github").unwrap(), None);
        assert_eq!(store.get("github:refresh").unwrap(), None);
    }

    #[tokio::test]
    async fn test_valid_token_refreshes_on_401() {
        let mut server = mockito::Server::new_async().await;
        let _validate_old = server
            .mock("GET", "/user")
            .match_header("Authorization", "Bearer ghu_expired")
            .with_status(401)
            .create_async()
            .await;
        let _validate_new = server
            .mock("GET", "/user")
            .match_header("Authorization", "Bearer ghu_refreshed")
            .with_status(200)
            .with_body(r#"{"login":"test","avatar_url":"https://example.com"}"#)
            .create_async()
            .await;
        let _refresh = server
            .mock("POST", "/login/oauth/access_token")
            .with_status(200)
            .with_body(
                r#"{
                "access_token": "ghu_refreshed",
                "token_type": "bearer",
                "scope": "",
                "refresh_token": "ghr_rotated",
                "refresh_token_expires_in": 15897600
            }"#,
            )
            .create_async()
            .await;

        let store = Arc::new(MockTokenStore::new());
        store.store("github", "ghu_expired").unwrap();
        store.store("github:refresh", "ghr_old").unwrap();

        let auth = Authenticator::with_github_api_base(
            #[expect(clippy::as_conversions, reason = "safe trait object upcast in test")]
            {
                Arc::clone(&store) as Arc<dyn TokenStore>
            },
            &server.url(),
        );

        let result = auth.valid_token("github").await.unwrap();
        assert_eq!(result, Some("ghu_refreshed".to_string()));
        assert_eq!(
            store.get("github").unwrap(),
            Some("ghu_refreshed".to_string())
        );
        assert_eq!(
            store.get("github:refresh").unwrap(),
            Some("ghr_rotated".to_string())
        );
    }

    #[tokio::test]
    async fn test_delete_token_also_clears_refresh() {
        let store = Arc::new(MockTokenStore::new());
        store.store("github", "ghu_access").unwrap();
        store.store("github:refresh", "ghr_refresh").unwrap();

        let auth = Authenticator::new(
            #[expect(clippy::as_conversions, reason = "safe trait object upcast in test")]
            {
                Arc::clone(&store) as Arc<dyn TokenStore>
            },
        );

        auth.delete_token("github").unwrap();
        assert_eq!(store.get("github").unwrap(), None);
        assert_eq!(store.get("github:refresh").unwrap(), None);
    }
}
