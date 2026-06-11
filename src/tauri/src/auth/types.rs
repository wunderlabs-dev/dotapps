//! Authentication types
//!
//! Domain types for OAuth flows and Git provider APIs.

use serde::{Deserialize, Serialize};

/// GitHub Device Flow response when starting OAuth
#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
}

/// GitHub OAuth token response
#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct AccessTokenResponse {
    pub access_token: Option<String>,
    pub token_type: Option<String>,
    pub scope: Option<String>,
    pub refresh_token: Option<String>,
    pub refresh_token_expires_in: Option<u64>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// GitHub user information
#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct GitHubUser {
    pub login: String,
    pub avatar_url: String,
}

/// GitHub repository information
#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
pub struct GitHubRepo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub html_url: String,
    pub clone_url: String,
    pub description: Option<String>,
    pub default_branch: String,
}

/// Whether a repository URL maps to a supported provider
#[derive(Clone, Debug, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSupport {
    pub provider: Option<String>,
    pub supported: bool,
    pub message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_code_response_serde() {
        let json = r#"{
            "device_code": "abc123",
            "user_code": "ABCD-1234",
            "verification_uri": "https://github.com/login/device",
            "expires_in": 900,
            "interval": 5
        }"#;

        let response: DeviceCodeResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.device_code, "abc123");
        assert_eq!(response.user_code, "ABCD-1234");
        assert_eq!(response.expires_in, 900);
    }

    #[test]
    fn test_access_token_response_success() {
        let json = r#"{
            "access_token": "token123",
            "token_type": "bearer",
            "scope": "repo"
        }"#;

        let response: AccessTokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.access_token, Some("token123".to_string()));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_access_token_response_with_refresh_token() {
        let json = r#"{
            "access_token": "ghu_token123",
            "token_type": "bearer",
            "scope": "",
            "refresh_token": "ghr_refresh456",
            "refresh_token_expires_in": 15897600,
            "expires_in": 28800
        }"#;

        let response: AccessTokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.access_token, Some("ghu_token123".to_string()));
        assert_eq!(response.refresh_token, Some("ghr_refresh456".to_string()));
        assert_eq!(response.refresh_token_expires_in, Some(15_897_600));
    }

    #[test]
    fn test_access_token_response_without_refresh_token() {
        let json = r#"{
            "access_token": "gho_legacy_token",
            "token_type": "bearer",
            "scope": "repo"
        }"#;

        let response: AccessTokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.access_token, Some("gho_legacy_token".to_string()));
        assert_eq!(response.refresh_token, None);
        assert_eq!(response.refresh_token_expires_in, None);
    }

    #[test]
    fn test_access_token_response_pending() {
        let json = r#"{
            "error": "authorization_pending",
            "error_description": "User has not yet authorized"
        }"#;

        let response: AccessTokenResponse = serde_json::from_str(json).unwrap();
        assert!(response.access_token.is_none());
        assert_eq!(response.error, Some("authorization_pending".to_string()));
    }

    #[test]
    fn test_github_user_serde() {
        let json = r#"{
            "login": "testuser",
            "avatar_url": "https://avatars.github.com/u/123"
        }"#;

        let user: GitHubUser = serde_json::from_str(json).unwrap();
        assert_eq!(user.login, "testuser");
    }

    #[test]
    fn test_github_repo_serde() {
        let json = r#"{
            "id": 12345,
            "name": "my-repo",
            "full_name": "user/my-repo",
            "private": false,
            "html_url": "https://github.com/user/my-repo",
            "clone_url": "https://github.com/user/my-repo.git",
            "description": "A test repo",
            "default_branch": "main"
        }"#;

        let repo: GitHubRepo = serde_json::from_str(json).unwrap();
        assert_eq!(repo.id, 12345);
        assert_eq!(repo.name, "my-repo");
        assert!(!repo.private);
    }

    #[test]
    fn test_github_repo_null_description() {
        let json = r#"{
            "id": 12345,
            "name": "my-repo",
            "full_name": "user/my-repo",
            "private": true,
            "html_url": "https://github.com/user/my-repo",
            "clone_url": "https://github.com/user/my-repo.git",
            "description": null,
            "default_branch": "main"
        }"#;

        let repo: GitHubRepo = serde_json::from_str(json).unwrap();
        assert!(repo.description.is_none());
        assert!(repo.private);
    }
}
