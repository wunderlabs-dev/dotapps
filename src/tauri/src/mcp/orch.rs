//! `OrchestratorOps` trait abstraction over `ProjectOrchestrator`.
//!
//! Lets `rollback_project` (Task 5.4) and `fork_project` (Task 6.2) be
//! tested without a live orchestrator.

use std::sync::Arc;

use async_trait::async_trait;
use tauri::AppHandle;

use crate::error::AppError;
use crate::projects::ProjectOrchestrator;

#[async_trait]
pub trait OrchestratorOps: Send + Sync {
    async fn force_stop_and_remove(&self, project_id: &str) -> Result<(), AppError>;
    async fn start_project(
        &self,
        app: &AppHandle,
        project_id: &str,
        port: u16,
    ) -> Result<(), AppError>;
}

pub struct LiveOrchestrator {
    inner: Arc<ProjectOrchestrator>,
}

impl LiveOrchestrator {
    pub fn new(inner: Arc<ProjectOrchestrator>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl OrchestratorOps for LiveOrchestrator {
    async fn force_stop_and_remove(&self, project_id: &str) -> Result<(), AppError> {
        self.inner.force_stop_and_remove(project_id).await
    }

    async fn start_project(
        &self,
        app: &AppHandle,
        project_id: &str,
        port: u16,
    ) -> Result<(), AppError> {
        self.inner.start_project(app, project_id, port).await
    }
}
