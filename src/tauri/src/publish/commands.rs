//! Publish Tauri commands

use std::sync::Arc;

use tauri::{Emitter, State};

use crate::auth::Authenticator;
use crate::error::AppError;
use crate::infrastructure::git::GitOps;
use crate::infrastructure::runtime::Runtime;
use crate::projects::orchestrator::StatusEvent;
use crate::projects::store::ProjectStore;
use crate::projects::types::ProjectId;
use crate::publish::deployer::{self, PublishDeps, UnpublishDeps};
use crate::publish::github_pages::GitHubPagesClient;
use crate::publish::types::PublishResponse;

/// Emit a project status event, logging on failure.
fn emit_publish_status(app: &tauri::AppHandle, project_id: &ProjectId, payload: &StatusEvent) {
    let event_name = crate::constants::events::project_status(project_id.as_str());
    if let Err(e) = app.emit(&event_name, payload) {
        tracing::warn!("cannot emit publish status event: {e}");
    }
}

#[tauri::command]
#[specta::specta]
pub async fn publish_project(
    app: tauri::AppHandle,
    store: State<'_, Arc<dyn ProjectStore>>,
    runtime: State<'_, Arc<dyn Runtime>>,
    git: State<'_, Arc<dyn GitOps>>,
    authenticator: State<'_, Arc<Authenticator>>,
    pages_client: State<'_, Arc<GitHubPagesClient>>,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    projectId: String,
) -> Result<PublishResponse, AppError> {
    let project_id = ProjectId::new(&projectId)?;

    let deps = PublishDeps {
        store: store.inner().as_ref(),
        runtime: runtime.inner().as_ref(),
        git: git.inner().as_ref(),
        authenticator: authenticator.inner().as_ref(),
        pages_client: pages_client.inner().as_ref(),
    };

    let (pages_url, _framework) = deployer::publish(&project_id, &deps).await?;

    let project = store.get(&project_id)?;
    let payload = StatusEvent::from_project(&project, Some(pages_url.clone()));
    emit_publish_status(&app, &project_id, &payload);

    Ok(PublishResponse { url: pages_url })
}

#[tauri::command]
#[specta::specta]
pub async fn unpublish_project(
    app: tauri::AppHandle,
    store: State<'_, Arc<dyn ProjectStore>>,
    authenticator: State<'_, Arc<Authenticator>>,
    pages_client: State<'_, Arc<GitHubPagesClient>>,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    projectId: String,
) -> Result<(), AppError> {
    let project_id = ProjectId::new(&projectId)?;

    let deps = UnpublishDeps {
        store: store.inner().as_ref(),
        authenticator: authenticator.inner().as_ref(),
        pages_client: pages_client.inner().as_ref(),
    };

    deployer::unpublish(&project_id, &deps).await?;

    let project = store.get(&project_id)?;
    let payload = StatusEvent::from_project(&project, None);
    emit_publish_status(&app, &project_id, &payload);

    Ok(())
}
