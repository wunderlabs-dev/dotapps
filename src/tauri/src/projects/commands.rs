//! Project Tauri commands

use std::sync::Arc;

use serde::Serialize;
use tauri::{Emitter, State};

use crate::constants::{events, install};
use crate::error::AppError;
use crate::infrastructure::{CloneResult, GitOps, Runtime};
use crate::projects::cursor;
use crate::projects::store::ProjectStore;
use crate::projects::syncer::ProjectSyncer;
use crate::projects::types::{
    InstallStep, InstallStepEvent, InstallStepStatus, Project, ProjectId, ProjectIntent,
};

/// Application state returned to frontend
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub projects: Vec<Project>,
    pub next_port: u16,
}

// ==================== State Commands ====================

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn state(store: State<Arc<dyn ProjectStore>>) -> Result<AppState, AppError> {
    let projects = store.list()?;
    let next_port = store.next_port()?;
    Ok(AppState {
        projects,
        next_port,
    })
}

// ==================== CRUD Commands ====================

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn list_projects(store: State<Arc<dyn ProjectStore>>) -> Result<Vec<Project>, AppError> {
    store.list()
}

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn project(store: State<Arc<dyn ProjectStore>>, id: String) -> Result<Project, AppError> {
    let project_id = ProjectId::new(&id)?;
    store.get(&project_id)
}

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn add_project(store: State<Arc<dyn ProjectStore>>, project: Project) -> Result<(), AppError> {
    store.add(project)
}

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn update_project(
    store: State<Arc<dyn ProjectStore>>,
    project: Project,
) -> Result<(), AppError> {
    store.update(project)
}

/// Dependencies for full project removal (container, pages, tunnel, files).
struct RemovalDeps<'deps> {
    store: &'deps dyn ProjectStore,
    orchestrator: &'deps crate::projects::ProjectOrchestrator,
    syncer: &'deps ProjectSyncer,
    authenticator: &'deps crate::auth::Authenticator,
    pages_client: &'deps crate::publish::github_pages::GitHubPagesClient,
    tunnel_coordinator: &'deps crate::tunnel::TunnelCoordinator,
}

async fn remove_project_with_cleanup(
    deps: &RemovalDeps<'_>,
    project_id: &ProjectId,
) -> Result<(), AppError> {
    let project = deps.store.get(project_id)?;
    let id_str = project_id.as_str();

    // Stop container and periodic sync
    deps.syncer.stop_periodic(id_str).await;
    let _ = deps.orchestrator.force_stop_and_remove(id_str).await;

    // Unpublish GitHub Pages (best-effort)
    if project.pages_url.is_some() {
        let unpublish_deps = crate::publish::deployer::UnpublishDeps {
            store: deps.store,
            authenticator: deps.authenticator,
            pages_client: deps.pages_client,
        };
        if let Err(e) = crate::publish::deployer::unpublish(project_id, &unpublish_deps).await {
            tracing::warn!("cannot unpublish pages on delete for {id_str}: {e}");
        }
    }

    // Tear down tunnel (best-effort)
    if project.tunnel_id.is_some() {
        let token = deps
            .authenticator
            .resolve_push_token(&project.repo_url)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        if let Err(e) = deps
            .tunnel_coordinator
            .unshare(project_id, &token, deps.store)
            .await
        {
            tracing::warn!("cannot tear down tunnel on delete for {id_str}: {e}");
        }
    }

    // Remove from store (metadata)
    deps.store.remove(project_id)?;

    // Delete repo files from disk (best-effort)
    let repo_path = project.repo_path();
    if repo_path.exists() {
        if let Err(e) = std::fs::remove_dir_all(&repo_path) {
            tracing::warn!("cannot delete repo files at {}: {e}", repo_path.display());
        }
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn remove_project(
    store: State<'_, Arc<dyn ProjectStore>>,
    orchestrator: State<'_, Arc<crate::projects::ProjectOrchestrator>>,
    syncer: State<'_, Arc<ProjectSyncer>>,
    authenticator: State<'_, Arc<crate::auth::Authenticator>>,
    pages_client: State<'_, Arc<crate::publish::github_pages::GitHubPagesClient>>,
    tunnel_coordinator: State<'_, Arc<crate::tunnel::TunnelCoordinator>>,
    id: String,
) -> Result<(), AppError> {
    let project_id = ProjectId::new(&id)?;
    let deps = RemovalDeps {
        store: store.inner().as_ref(),
        orchestrator: orchestrator.inner().as_ref(),
        syncer: syncer.inner().as_ref(),
        authenticator: authenticator.inner().as_ref(),
        pages_client: pages_client.inner().as_ref(),
        tunnel_coordinator: tunnel_coordinator.inner().as_ref(),
    };
    remove_project_with_cleanup(&deps, &project_id).await
}

// ==================== Import Commands ====================

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn import_project(
    importer: State<Arc<crate::projects::ProjectImporter>>,
    url: String,
    name: String,
    token: Option<String>,
) -> Result<Project, AppError> {
    importer.import(&url, &name, token.as_deref())
}

// ==================== Install Command ====================

#[tauri::command]
#[specta::specta]
pub async fn install_project(
    app: tauri::AppHandle,
    orchestrator: State<'_, Arc<crate::projects::ProjectOrchestrator>>,
    store: State<'_, Arc<dyn ProjectStore>>,
    syncer: State<'_, Arc<ProjectSyncer>>,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    projectId: String,
) -> Result<u16, AppError> {
    let project_id = ProjectId::new(&projectId)?;

    // Guard: only for uninstalled projects
    let project = store.get(&project_id)?;
    if project.installed {
        return Err(AppError::InvalidInput {
            field: "projectId".to_string(),
            reason: "project is already installed".to_string(),
        });
    }

    let port = match project.port {
        Some(p) => p,
        None => syncer.allocate_port()?,
    };

    let runtime = orchestrator.runtime();
    run_install_pipeline(&runtime, &app, project_id.as_str(), &project, port).await?;

    // Mark as installed and hand off to the orchestrator for ongoing reconciliation
    {
        let mut project = store.get(&project_id)?;
        project.installed = true;
        project.intent = ProjectIntent::Run;
        project.port = Some(port);
        project.status = crate::projects::types::ProjectStatus::Ready;
        store.update(project)?;
    }

    // Spawn the reconciler so ongoing health checks continue
    orchestrator
        .start_project(&app, project_id.as_str(), port)
        .await?;
    syncer.start_periodic(&app, project_id.as_str()).await;

    Ok(port)
}

// ==================== Lifecycle Commands ====================

#[tauri::command]
#[specta::specta]
pub async fn start_project(
    app: tauri::AppHandle,
    orchestrator: State<'_, Arc<crate::projects::ProjectOrchestrator>>,
    store: State<'_, Arc<dyn ProjectStore>>,
    syncer: State<'_, Arc<ProjectSyncer>>,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    projectId: String,
) -> Result<u16, AppError> {
    let project_id = ProjectId::new(&projectId)?;

    // Guard: start_project requires project to be installed
    let project_check = store.get(&project_id)?;
    if !project_check.installed {
        return Err(AppError::InvalidInput {
            field: "projectId".to_string(),
            reason: "project must be installed first".to_string(),
        });
    }

    // Reuse the previously assigned port when one exists, because the stopped
    // container's port binding must match. Only allocate a fresh port for
    // projects that have never been started.
    let port = match store.get(&project_id) {
        Ok(project) if project.intent == crate::projects::types::ProjectIntent::Run => {
            // Already running with a port: return immediately (idempotency)
            if let Some(port) = project.port {
                return Ok(port);
            }
            syncer.allocate_port()?
        }
        Ok(project) if project.port.is_some() => {
            // Stopped but has a previously assigned port: reuse it
            project.port.expect("checked by guard")
        }
        _ => syncer.allocate_port()?,
    };

    orchestrator
        .start_project(&app, project_id.as_str(), port)
        .await?;

    syncer.start_periodic(&app, project_id.as_str()).await;

    Ok(port)
}

#[tauri::command]
#[specta::specta]
pub async fn stop_project(
    app: tauri::AppHandle,
    orchestrator: State<'_, Arc<crate::projects::ProjectOrchestrator>>,
    syncer: State<'_, Arc<ProjectSyncer>>,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    projectId: String,
) -> Result<(), AppError> {
    let project_id = ProjectId::new(&projectId)?;
    syncer
        .stop_with_sync(&app, orchestrator.inner().as_ref(), &project_id)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn restart_project(
    app: tauri::AppHandle,
    orchestrator: State<'_, Arc<crate::projects::ProjectOrchestrator>>,
    syncer: State<'_, Arc<ProjectSyncer>>,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    projectId: String,
) -> Result<u16, AppError> {
    let project_id = ProjectId::new(&projectId)?;

    syncer.stop_periodic(project_id.as_str()).await;
    orchestrator
        .force_stop_and_remove(project_id.as_str())
        .await?;

    let port = syncer.allocate_port()?;
    orchestrator
        .start_project(&app, project_id.as_str(), port)
        .await?;

    syncer.start_periodic(&app, project_id.as_str()).await;

    Ok(port)
}

// ==================== IDE Commands ====================

#[tauri::command]
#[specta::specta]
pub fn is_cursor_installed() -> bool {
    cursor::cursor_binary_installed()
}

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn open_project_in_cursor(
    store: State<'_, Arc<dyn ProjectStore>>,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    projectId: String,
) -> Result<bool, AppError> {
    let project_id = ProjectId::new(&projectId)?;
    let project = store.get(&project_id)?;
    let repo_path = project.repo_path();
    cursor::open_in_cursor(&repo_path, &project, project.port)
}

// ==================== Validation Helpers ====================

fn validate_repo_url(url: &str) -> Result<(), AppError> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput {
            field: "repoUrl".into(),
            reason: "cannot use empty repository url".into(),
        });
    }
    if trimmed.starts_with("file://") {
        return Err(AppError::InvalidInput {
            field: "repoUrl".into(),
            reason: "cannot use local file url: use a remote url (https or ssh)".into(),
        });
    }
    if trimmed.starts_with("https://")
        || trimmed.starts_with("http://")
        || trimmed.starts_with("git@")
        || trimmed.starts_with("ssh://")
    {
        return Ok(());
    }
    Err(AppError::InvalidInput {
        field: "repoUrl".into(),
        reason: "cannot use unsupported url scheme: use https://, http://, ssh://, or git@..."
            .into(),
    })
}

fn validate_branch_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::InvalidInput {
            field: "branchName".into(),
            reason: "cannot use empty branch name".into(),
        });
    }
    if name.trim().is_empty() {
        return Err(AppError::InvalidInput {
            field: "branchName".into(),
            reason: "cannot use blank branch name".into(),
        });
    }
    if name.contains("..") {
        return Err(AppError::InvalidInput {
            field: "branchName".into(),
            reason: "cannot use branch name with path traversal".into(),
        });
    }
    if name.chars().any(|c| c.is_ascii_control()) {
        return Err(AppError::InvalidInput {
            field: "branchName".into(),
            reason: "cannot use branch name with control characters".into(),
        });
    }
    Ok(())
}

// ==================== Git Commands ====================

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn clone_repository(
    store: State<'_, Arc<dyn ProjectStore>>,
    git: State<'_, Arc<dyn GitOps>>,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    repoUrl: String,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    projectId: String,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    accessToken: Option<String>,
) -> Result<CloneResult, AppError> {
    validate_repo_url(&repoUrl)?;
    let project_id = ProjectId::new(projectId)?;
    let repo_path = project_id.repo_path()?;
    if repo_path.exists() {
        return Err(AppError::AlreadyExists {
            entity: "project".into(),
            id: project_id.to_string(),
        });
    }
    if let Some(parent) = repo_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let result = git.clone_repo(&repoUrl, &repo_path, accessToken.as_deref())?;
    let _slug = crate::projects::importer::after_clone_install(
        store.inner().as_ref(),
        &repoUrl,
        std::path::Path::new(&result.local_path),
    )?;
    Ok(result)
}

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn pull_repository(
    store: State<'_, Arc<dyn ProjectStore>>,
    git: State<'_, Arc<dyn GitOps>>,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    projectId: String,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    accessToken: Option<String>,
) -> Result<(), AppError> {
    let project_id = ProjectId::new(projectId)?;
    let project = store.get(&project_id)?;
    git.pull(&project.repo_path(), accessToken.as_deref())
}

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn checkout_branch(
    store: State<'_, Arc<dyn ProjectStore>>,
    git: State<'_, Arc<dyn GitOps>>,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    projectId: String,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    branchName: String,
) -> Result<(), AppError> {
    validate_branch_name(&branchName)?;
    let project_id = ProjectId::new(projectId)?;
    let project = store.get(&project_id)?;
    git.checkout(&project.repo_path(), &branchName, false)
}

#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri command handler receives owned deserialized values"
)]
pub fn list_branches(
    store: State<'_, Arc<dyn ProjectStore>>,
    git: State<'_, Arc<dyn GitOps>>,
    #[expect(
        non_snake_case,
        reason = "Tauri deserializes camelCase from JS frontend"
    )]
    projectId: String,
) -> Result<Vec<String>, AppError> {
    let project_id = ProjectId::new(projectId)?;
    let project = store.get(&project_id)?;
    git.list_branches(&project.repo_path())
}

// ==================== Install Helpers ====================

/// Poll a condition at a fixed interval until it returns `true` or attempts are exhausted.
async fn poll_until<F, Fut>(check: F, max_attempts: u32, interval: std::time::Duration) -> bool
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    for _ in 0..max_attempts {
        if check().await {
            return true;
        }
        tokio::time::sleep(interval).await;
    }
    false
}

/// Wait for the runtime (VM) to become reachable.
async fn wait_for_runtime(runtime: &Arc<dyn Runtime>) -> bool {
    let runtime = Arc::clone(runtime);
    poll_until(
        || {
            let runtime = Arc::clone(&runtime);
            async move { runtime.is_reachable().await }
        },
        install::VM_READY_MAX_ATTEMPTS,
        install::POLL_INTERVAL,
    )
    .await
}

/// Wait for the dev server to respond on HTTP.
async fn wait_for_http_ready(port: u16) -> bool {
    poll_until(
        || super::orchestrator::probe_http_health(port),
        install::HTTP_READY_MAX_ATTEMPTS,
        install::POLL_INTERVAL,
    )
    .await
}

/// Execute the install pipeline: VM readiness, npm install, dev server, HTTP check.
async fn run_install_pipeline(
    runtime: &Arc<dyn Runtime>,
    app: &tauri::AppHandle,
    project_id: &str,
    project: &Project,
    port: u16,
) -> Result<(), AppError> {
    let emit_step = |step: &InstallStep, status: &InstallStepStatus| {
        let event_name = events::project_install_step(project_id);
        let payload = InstallStepEvent {
            step: step.clone(),
            status: status.clone(),
        };
        let _ = app.emit(&event_name, &payload);
    };
    // Step 1: Wait for VM/runtime to be reachable
    emit_step(&InstallStep::VmStarting, &InstallStepStatus::Active);
    let reachable = wait_for_runtime(runtime).await;
    if !reachable {
        emit_step(
            &InstallStep::VmStarting,
            &InstallStepStatus::Failed {
                reason: "VM failed to start".to_string(),
            },
        );
        return Err(AppError::VmNotRunning);
    }
    emit_step(&InstallStep::VmStarting, &InstallStepStatus::Done);

    // Step 2: Run npm install (one-shot container, exits when done)
    tracing::info!("install {project_id}: starting npm install (one-shot container)");
    emit_step(
        &InstallStep::AllocatingResources,
        &InstallStepStatus::Active,
    );
    let install_id = crate::constants::containers::install_name(project_id);
    let install_result = runtime.run_install(project_id, &project.repo_path()).await;
    let _ = runtime.remove(&install_id).await;
    if let Err(e) = install_result {
        tracing::error!("install {project_id}: npm install failed: {e}");
        emit_step(
            &InstallStep::AllocatingResources,
            &InstallStepStatus::Failed {
                reason: e.to_string(),
            },
        );
        return Err(e);
    }
    tracing::info!("install {project_id}: npm install completed");
    emit_step(&InstallStep::AllocatingResources, &InstallStepStatus::Done);

    // Step 3: Start dev server container
    tracing::info!("install {project_id}: starting dev server on port {port}");
    emit_step(&InstallStep::NpmInstalling, &InstallStepStatus::Active);
    if let Err(e) = runtime.start(project_id, &project.repo_path(), port).await {
        tracing::error!("install {project_id}: dev server start failed: {e}");
        emit_step(
            &InstallStep::NpmInstalling,
            &InstallStepStatus::Failed {
                reason: e.to_string(),
            },
        );
        cleanup_on_failure(runtime, project_id).await;
        return Err(e);
    }
    tracing::info!("install {project_id}: dev server container started");
    emit_step(&InstallStep::NpmInstalling, &InstallStepStatus::Done);

    // Step 4: Wait for dev server to respond on HTTP
    tracing::info!("install {project_id}: waiting for HTTP on port {port}");
    emit_step(&InstallStep::RunningServer, &InstallStepStatus::Active);
    if !wait_for_http_ready(port).await {
        emit_step(
            &InstallStep::RunningServer,
            &InstallStepStatus::Failed {
                reason: "dev server did not become ready".to_string(),
            },
        );
        cleanup_on_failure(runtime, project_id).await;
        return Err(AppError::ContainerFailed {
            reason: "dev server did not become ready".to_string(),
        });
    }
    tracing::info!("install {project_id}: server ready on port {port}");
    emit_step(&InstallStep::RunningServer, &InstallStepStatus::Done);
    Ok(())
}

/// Clean up container on install failure.
async fn cleanup_on_failure(runtime: &Arc<dyn Runtime>, project_id: &str) {
    if let Err(e) = runtime.stop(project_id).await {
        tracing::debug!("cleanup: cannot stop container {project_id}: {e}");
    }
    if let Err(e) = runtime.remove(project_id).await {
        tracing::debug!("cleanup: cannot remove container {project_id}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_branch_name, validate_repo_url};

    #[test]
    fn test_validate_repo_url_accepts_https() {
        assert!(validate_repo_url("https://github.com/foo/bar.git").is_ok());
        assert!(validate_repo_url("https://example.com/repo").is_ok());
    }

    #[test]
    fn test_validate_repo_url_accepts_http() {
        assert!(validate_repo_url("http://example.com/repo.git").is_ok());
    }

    #[test]
    fn test_validate_repo_url_accepts_ssh() {
        assert!(validate_repo_url("ssh://git@github.com/user/repo.git").is_ok());
        assert!(validate_repo_url("git@github.com:user/repo.git").is_ok());
    }

    #[test]
    fn test_validate_repo_url_rejects_empty() {
        assert!(validate_repo_url("").is_err());
        assert!(validate_repo_url("   ").is_err());
    }

    #[test]
    fn test_validate_repo_url_rejects_file() {
        assert!(validate_repo_url("file:///path/to/repo").is_err());
        assert!(validate_repo_url("file://localhost/path").is_err());
    }

    #[test]
    fn test_validate_repo_url_rejects_unsupported_scheme() {
        assert!(validate_repo_url("ftp://example.com/repo").is_err());
        assert!(validate_repo_url("xyz://foo").is_err());
    }

    #[test]
    fn test_validate_branch_name_accepts_normal() {
        assert!(validate_branch_name("main").is_ok());
        assert!(validate_branch_name("feature/foo").is_ok());
        assert!(validate_branch_name("release-v1.2").is_ok());
    }

    #[test]
    fn test_validate_branch_name_rejects_empty() {
        assert!(validate_branch_name("").is_err());
    }

    #[test]
    fn test_validate_branch_name_rejects_blank() {
        assert!(validate_branch_name("   ").is_err());
        assert!(validate_branch_name("\t").is_err());
    }

    #[test]
    fn test_validate_branch_name_rejects_path_traversal() {
        assert!(validate_branch_name("..").is_err());
        assert!(validate_branch_name("foo/../bar").is_err());
    }

    #[test]
    fn test_validate_branch_name_rejects_control_chars() {
        assert!(validate_branch_name("main\x00").is_err());
        assert!(validate_branch_name("\n").is_err());
    }
}
