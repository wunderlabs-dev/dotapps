//! Tauri command handlers for authentication
//!
//! Thin wrappers that delegate to the Authenticator service.

use super::authenticator::Authenticator;
use super::types::{
    AccessTokenResponse, DeviceCodeResponse, GitHubRepo, GitHubUser, ProviderSupport,
};
use crate::error::AppError;

/// Store an authentication token for a provider
#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn store_auth_token(
    authenticator: tauri::State<'_, std::sync::Arc<Authenticator>>,
    provider: String,
    token: String,
) -> Result<(), AppError> {
    authenticator.store_token(&provider, &token)
}

/// Get an authentication token for a provider
#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn auth_token(
    authenticator: tauri::State<'_, std::sync::Arc<Authenticator>>,
    provider: String,
) -> Result<Option<String>, AppError> {
    authenticator.token(&provider)
}

/// Delete an authentication token for a provider
#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn delete_auth_token(
    authenticator: tauri::State<'_, std::sync::Arc<Authenticator>>,
    provider: String,
) -> Result<(), AppError> {
    authenticator.delete_token(&provider)
}

/// Determine the Git provider from a repository URL
#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn provider_from_url(url: String) -> Option<String> {
    Authenticator::provider_from_url(&url).map(std::string::ToString::to_string)
}

/// Start GitHub Device OAuth flow
#[tauri::command]
#[specta::specta]
pub async fn github_start_oauth(
    authenticator: tauri::State<'_, std::sync::Arc<Authenticator>>,
    client_id: Option<String>,
) -> Result<DeviceCodeResponse, AppError> {
    authenticator.github_start_oauth(client_id.as_deref()).await
}

/// Poll GitHub for OAuth access token
#[tauri::command]
#[specta::specta]
pub async fn github_poll_oauth(
    authenticator: tauri::State<'_, std::sync::Arc<Authenticator>>,
    device_code: String,
    client_id: Option<String>,
) -> Result<AccessTokenResponse, AppError> {
    authenticator
        .github_poll_oauth(&device_code, client_id.as_deref())
        .await
}

/// Get current GitHub user information
#[tauri::command]
#[specta::specta]
pub async fn github_user(
    authenticator: tauri::State<'_, std::sync::Arc<Authenticator>>,
    token: String,
) -> Result<GitHubUser, AppError> {
    authenticator.github_user(&token).await
}

/// List user's GitHub repositories
#[tauri::command]
#[specta::specta]
pub async fn github_list_repos(
    authenticator: tauri::State<'_, std::sync::Arc<Authenticator>>,
    token: String,
    page: Option<u32>,
    per_page: Option<u32>,
) -> Result<Vec<GitHubRepo>, AppError> {
    authenticator
        .github_list_repos(&token, page.unwrap_or(1), per_page.unwrap_or(30))
        .await
}

/// Validate a stored authentication token against the provider API
#[tauri::command]
#[specta::specta]
pub async fn validate_auth_token(
    authenticator: tauri::State<'_, std::sync::Arc<Authenticator>>,
    provider: String,
) -> Result<bool, AppError> {
    Ok(authenticator.valid_token(&provider).await?.is_some())
}

/// Check whether a repository URL maps to a supported provider
#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn check_provider_support(url: String) -> ProviderSupport {
    if let Some(provider) = Authenticator::provider_from_url(&url) {
        ProviderSupport {
            provider: Some(provider.to_string()),
            supported: true,
            message: None,
        }
    } else if let Some(name) = Authenticator::is_unsupported_provider(&url) {
        ProviderSupport {
            provider: Some(name.to_lowercase()),
            supported: false,
            message: Some(format!(
                "{name} is not yet supported. GitHub repos work today."
            )),
        }
    } else {
        ProviderSupport {
            provider: None,
            supported: false,
            message: Some("Could not detect a git provider from this URL.".to_string()),
        }
    }
}
