//! WorkspaceManager - global state for managing multiple workspaces.
//!
//! This is the entry point for all workspace operations, providing
//! thread-safe access to workspace runtimes.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api::types::{ApiError, WorkspaceSummary};
use crate::runtime::path::canonicalize_workspace_root;
use crate::runtime::workspace::WorkspaceRuntime;

/// Global manager for all workspaces.
///
/// Thread-safe: Uses tokio::sync::Mutex for concurrent access.
/// Injected into Tauri as managed state via `app.manage()`.
pub struct WorkspaceManager {
    /// Map of workspace ID to runtime
    workspaces: Mutex<HashMap<String, Arc<WorkspaceRuntime>>>,
}

impl WorkspaceManager {
    /// Creates a new WorkspaceManager.
    pub fn new() -> Self {
        log::debug!("Initializing WorkspaceManager");
        Self {
            workspaces: Mutex::new(HashMap::new()),
        }
    }

    /// Creates a new workspace with the given root directory.
    ///
    /// # Arguments
    /// * `root_dir` - Path to workspace root (will be canonicalized)
    ///
    /// # Returns
    /// * `Ok(WorkspaceSummary)` - Summary of created workspace
    /// * `Err(ApiError)` - If path validation fails
    pub async fn create_workspace(&self, root_dir: &str) -> Result<WorkspaceSummary, ApiError> {
        if root_dir.trim().is_empty() {
            return Err(ApiError::InvalidInput {
                message: "Root directory cannot be empty".to_string(),
            });
        }

        // Validate and canonicalize the path
        let canonical_root = canonicalize_workspace_root(root_dir)?;

        // Generate unique workspace ID
        let workspace_id = Uuid::new_v4().to_string();

        // Create the runtime
        let runtime = Arc::new(WorkspaceRuntime::new(
            workspace_id.clone(),
            canonical_root.clone(),
        ));

        // Get summary before inserting (avoids holding lock during summary creation)
        let summary = runtime.summary();

        // Insert into map
        {
            let mut workspaces = self.workspaces.lock().await;
            workspaces.insert(workspace_id.clone(), runtime);
            log::info!(
                "Workspace created: id={workspace_id}, total_workspaces={}",
                workspaces.len()
            );
        }

        Ok(summary)
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_create_workspace() {
        let manager = WorkspaceManager::new();
        let temp_dir = env::temp_dir();

        let result = manager.create_workspace(temp_dir.to_str().unwrap()).await;

        assert!(result.is_ok());
        let summary = result.unwrap();
        assert!(!summary.workspace_id.is_empty());
        assert!(summary.created_at_ms > 0.0);
    }

    #[tokio::test]
    async fn test_create_workspace_invalid_path() {
        let manager = WorkspaceManager::new();

        let result = manager.create_workspace("/nonexistent/path/12345").await;

        assert!(matches!(result, Err(ApiError::PathNotFound { .. })));
    }

    #[tokio::test]
    async fn test_create_workspace_empty_root_dir() {
        let manager = WorkspaceManager::new();

        let result = manager.create_workspace("").await;

        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));
    }
}
