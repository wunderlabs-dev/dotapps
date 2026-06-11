//! Tunnel sharing Tauri commands

use std::sync::Arc;

use serde::Serialize;
use tauri::{Emitter, State};

use crate::auth::Authenticator;
use crate::error::AppError;
use crate::projects::orchestrator::StatusEvent;
use crate::projects::store::ProjectStore;
use crate::projects::types::{ProjectId, ProjectStatus};
use crate::tunnel::TunnelCoordinator;

/// Response returned to the frontend after sharing a project
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ShareResponse {
    pub url: String,
}

#[tauri::command]
#[specta::specta]
pub async fn share_project(
    app: tauri::AppHandle,
    store: State<'_, Arc<dyn ProjectStore>>,
    tunnel_coordinator: State<'_, Arc<TunnelCoordinator>>,
    authenticator: State<'_, Arc<Authenticator>>,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    projectId: String,
) -> Result<ShareResponse, AppError> {
    let project_id = ProjectId::new(&projectId)?;
    let project = store.get(&project_id)?;

    // Validate the project is ready and has an assigned port
    if project.status != ProjectStatus::Ready {
        return Err(AppError::TunnelFailed {
            reason: format!(
                "cannot share project {projectId}: not ready (status: {})",
                project.status
            ),
        });
    }
    let port = project.port.ok_or_else(|| AppError::TunnelFailed {
        reason: format!("cannot share project {projectId}: no assigned port"),
    })?;

    // Resolve GitHub token for tunnel API authentication
    let github_token = authenticator
        .resolve_push_token(&project.repo_url)
        .await?
        .ok_or_else(|| AppError::AuthRequired {
            provider: "github".into(),
        })?;

    let url = tunnel_coordinator
        .share(
            &project_id,
            &project.name,
            port,
            &github_token,
            store.inner().as_ref(),
        )
        .await?;

    let project = store.get(&project_id)?;
    let event_name = crate::constants::events::project_status(project_id.as_str());
    let payload = StatusEvent {
        status: project.status,
        port: project.port,
        error_summary: None,
        fix_attempt: None,
        tunnel_url: Some(url.clone()),
        tunnel_live: Some(true),
        pages_url: project.pages_url.clone(),
    };
    let _ = app.emit(&event_name, &payload);

    Ok(ShareResponse { url })
}

#[tauri::command]
#[specta::specta]
pub async fn unshare_project(
    app: tauri::AppHandle,
    store: State<'_, Arc<dyn ProjectStore>>,
    tunnel_coordinator: State<'_, Arc<TunnelCoordinator>>,
    authenticator: State<'_, Arc<Authenticator>>,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    projectId: String,
) -> Result<(), AppError> {
    let project_id = ProjectId::new(&projectId)?;
    let project = store.get(&project_id)?;

    let github_token = authenticator
        .resolve_push_token(&project.repo_url)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    tunnel_coordinator
        .unshare(&project_id, &github_token, store.inner().as_ref())
        .await?;

    // Emit status event to immediately clear tunnel URL in frontend
    let project = store.get(&project_id)?;
    let event_name = crate::constants::events::project_status(project_id.as_str());
    let payload = StatusEvent {
        status: project.status,
        port: project.port,
        error_summary: None,
        fix_attempt: None,
        tunnel_url: None,
        tunnel_live: None,
        pages_url: project.pages_url.clone(),
    };
    let _ = app.emit(&event_name, &payload);

    Ok(())
}
