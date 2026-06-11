//! Cloudflare Worker API client for tunnel CRUD

use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Sent to the tunnel API when sharing a project
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTunnelRequest {
    pub project_name: String,
    pub project_id: String,
    pub host_port: u16,
}

/// Returned by the tunnel API after creating a tunnel and DNS record
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTunnelResponse {
    pub tunnel_id: String,
    pub tunnel_token: String,
    pub url: String,
}

/// Client for the Opnble tunnel API (Cloudflare Worker)
pub struct TunnelApiClient {
    http: reqwest::Client,
    base_url: String,
}

impl TunnelApiClient {
    pub fn new(base_url: &str) -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| AppError::TunnelFailed {
                reason: format!("cannot build HTTP client: {e}"),
            })?;
        Ok(Self {
            http,
            base_url: base_url.to_string(),
        })
    }

    /// Create a tunnel and DNS record, returning the token and public URL.
    /// Authenticates with the user's GitHub token.
    pub async fn create_tunnel(
        &self,
        project_name: &str,
        project_id: &str,
        host_port: u16,
        github_token: &str,
    ) -> Result<CreateTunnelResponse, AppError> {
        let url = format!("{}/tunnels", self.base_url);
        let body = CreateTunnelRequest {
            project_name: project_name.to_string(),
            project_id: project_id.to_string(),
            host_port,
        };

        let response = self
            .http
            .post(&url)
            .bearer_auth(github_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::TunnelFailed {
                reason: format!("cannot reach tunnel API: {e}"),
            })?;

        let response = check_response(response, "create tunnel").await?;

        response
            .json::<CreateTunnelResponse>()
            .await
            .map_err(|e| AppError::TunnelFailed {
                reason: format!("cannot parse tunnel API response: {e}"),
            })
    }

    /// Delete a tunnel and its DNS record.
    /// Authenticates with the user's GitHub token.
    pub async fn delete_tunnel(&self, tunnel_id: &str, github_token: &str) -> Result<(), AppError> {
        let url = format!("{}/tunnels/{tunnel_id}", self.base_url);

        let response = self
            .http
            .delete(&url)
            .bearer_auth(github_token)
            .send()
            .await
            .map_err(|e| AppError::TunnelFailed {
                reason: format!("cannot reach tunnel API: {e}"),
            })?;

        check_response(response, "delete tunnel").await?;

        Ok(())
    }
}

/// Validate that the HTTP response indicates success, returning the response
/// for further processing. Returns `TunnelFailed` with context on error.
async fn check_response(
    response: reqwest::Response,
    context: &str,
) -> Result<reqwest::Response, AppError> {
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::TunnelFailed {
            reason: format!("cannot {context}: API returned {status}: {body}"),
        });
    }
    Ok(response)
}
