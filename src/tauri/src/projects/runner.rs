//! Running container tracker for platform-specific status queries.
//!
//! Lifecycle management (start/stop/restart) has moved to `ProjectOrchestrator`.

use std::collections::HashMap;

use tokio::sync::Mutex;

use crate::projects::types::ProjectId;

/// Tracks running containers by project ID
pub struct RunningContainers {
    containers: Mutex<HashMap<ProjectId, String>>,
}

impl RunningContainers {
    pub fn new() -> Self {
        Self {
            containers: Mutex::new(HashMap::new()),
        }
    }

    #[allow(dead_code, reason = "used by platform/linux.rs container tracking")]
    pub async fn insert(&self, project_id: &ProjectId, container_id: String) {
        self.containers
            .lock()
            .await
            .insert(project_id.clone(), container_id);
    }

    #[allow(dead_code, reason = "used by platform/linux.rs container tracking")]
    pub async fn remove(&self, project_id: &ProjectId) -> Option<String> {
        self.containers.lock().await.remove(project_id)
    }

    #[allow(dead_code, reason = "used by platform/linux.rs container tracking")]
    pub async fn get(&self, project_id: &ProjectId) -> Option<String> {
        self.containers.lock().await.get(project_id).cloned()
    }

    pub async fn list(&self) -> Vec<(ProjectId, String)> {
        self.containers
            .lock()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

impl Default for RunningContainers {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::types::ProjectId;

    #[tokio::test]
    async fn test_running_containers() {
        let running = RunningContainers::new();
        let id = ProjectId::new("proj-1").unwrap();

        running.insert(&id, "container-abc".into()).await;
        assert_eq!(running.get(&id).await, Some("container-abc".into()));

        let removed = running.remove(&id).await;
        assert_eq!(removed, Some("container-abc".into()));
        assert_eq!(running.get(&id).await, None);
    }

    #[tokio::test]
    async fn test_running_containers_list_returns_project_ids() {
        let running = RunningContainers::new();
        let id = ProjectId::new("proj-1").unwrap();

        running.insert(&id, "container-abc".into()).await;
        let list = running.list().await;
        assert_eq!(list.len(), 1);
        let (listed_id, listed_container) = list.first().unwrap();
        assert_eq!(*listed_id, id);
        assert_eq!(listed_container, "container-abc");
    }
}
