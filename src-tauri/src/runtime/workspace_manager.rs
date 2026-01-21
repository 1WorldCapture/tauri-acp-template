//! WorkspaceManager - global state for managing multiple workspaces.
//!
//! This is the entry point for all workspace operations, providing
//! thread-safe access to workspace runtimes.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api::types::{AgentSummary, ApiError, WorkspaceId, WorkspaceSummary};
use crate::runtime::path::canonicalize_workspace_root;
use crate::runtime::workspace::WorkspaceRuntime;

/// Global manager for all workspaces.
///
/// Thread-safe: Uses tokio::sync::Mutex for concurrent access.
/// Injected into Tauri as managed state via `app.manage()`.
pub struct WorkspaceManager {
    /// Map of workspace ID to runtime
    workspaces: Mutex<HashMap<String, Arc<WorkspaceRuntime>>>,
    /// Currently focused workspace ID (UI state)
    focused_workspace_id: Mutex<Option<WorkspaceId>>,
}

impl WorkspaceManager {
    /// Creates a new WorkspaceManager.
    pub fn new() -> Self {
        log::debug!("Initializing WorkspaceManager");
        Self {
            workspaces: Mutex::new(HashMap::new()),
            focused_workspace_id: Mutex::new(None),
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

    /// Sets the currently focused workspace.
    ///
    /// # Arguments
    /// * `workspace_id` - ID of the workspace to focus
    ///
    /// # Returns
    /// * `Ok(())` - Focus was set successfully
    /// * `Err(ApiError::InvalidInput)` - If workspace_id is empty
    /// * `Err(ApiError::WorkspaceNotFound)` - If workspace does not exist
    pub async fn set_focus(&self, workspace_id: WorkspaceId) -> Result<(), ApiError> {
        if workspace_id.trim().is_empty() {
            return Err(ApiError::InvalidInput {
                message: "Workspace ID cannot be empty".to_string(),
            });
        }

        // Verify workspace exists (lock then drop before acquiring next lock)
        {
            let workspaces = self.workspaces.lock().await;
            if !workspaces.contains_key(&workspace_id) {
                return Err(ApiError::WorkspaceNotFound {
                    workspace_id: workspace_id.clone(),
                });
            }
        }

        // Set focus
        {
            let mut focused = self.focused_workspace_id.lock().await;
            *focused = Some(workspace_id.clone());
            log::info!("Workspace focus set: {workspace_id}");
        }

        Ok(())
    }

    /// Gets the currently focused workspace ID.
    ///
    /// # Returns
    /// * `Some(WorkspaceId)` - ID of the focused workspace
    /// * `None` - No workspace is currently focused
    pub async fn get_focus(&self) -> Option<WorkspaceId> {
        let focused = self.focused_workspace_id.lock().await;
        focused.clone()
    }

    /// Gets a workspace runtime by ID.
    ///
    /// # Arguments
    /// * `workspace_id` - ID of the workspace to get
    ///
    /// # Returns
    /// * `Ok(Arc<WorkspaceRuntime>)` - The workspace runtime
    /// * `Err(ApiError::WorkspaceNotFound)` - If the workspace does not exist
    pub async fn get_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Arc<WorkspaceRuntime>, ApiError> {
        let workspaces = self.workspaces.lock().await;
        workspaces
            .get(workspace_id)
            .cloned()
            .ok_or_else(|| ApiError::WorkspaceNotFound {
                workspace_id: workspace_id.clone(),
            })
    }

    /// Creates an agent entity within a workspace.
    ///
    /// # Arguments
    /// * `workspace_id` - ID of the workspace to create the agent in
    /// * `plugin_id` - Plugin identifier (e.g., "claude-code", "codex", "gemini")
    /// * `display_name` - Optional display name for the agent
    ///
    /// # Returns
    /// * `Ok(AgentSummary)` - Summary of the created agent
    /// * `Err(ApiError::WorkspaceNotFound)` - If the workspace does not exist
    /// * `Err(ApiError::InvalidInput)` - If validation fails
    pub async fn create_agent(
        &self,
        workspace_id: WorkspaceId,
        plugin_id: String,
        display_name: Option<String>,
    ) -> Result<AgentSummary, ApiError> {
        // Get workspace runtime (releases lock after clone)
        let workspace = self.get_workspace(&workspace_id).await?;

        // Delegate to workspace runtime
        workspace.create_agent(plugin_id, display_name).await
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

    #[tokio::test]
    async fn test_focus_default_none() {
        let manager = WorkspaceManager::new();

        let result = manager.get_focus().await;

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_set_and_get_focus() {
        let manager = WorkspaceManager::new();
        let temp_dir = env::temp_dir();

        // Create a workspace first
        let summary = manager
            .create_workspace(temp_dir.to_str().unwrap())
            .await
            .unwrap();

        // Set focus to the created workspace
        let result = manager.set_focus(summary.workspace_id.clone()).await;
        assert!(result.is_ok());

        // Verify focus is set
        let focused = manager.get_focus().await;
        assert_eq!(focused, Some(summary.workspace_id));
    }

    #[tokio::test]
    async fn test_set_focus_unknown_workspace() {
        let manager = WorkspaceManager::new();

        let result = manager
            .set_focus("nonexistent-workspace-id".to_string())
            .await;

        assert!(matches!(
            result,
            Err(ApiError::WorkspaceNotFound { workspace_id }) if workspace_id == "nonexistent-workspace-id"
        ));
    }

    #[tokio::test]
    async fn test_set_focus_empty_workspace_id() {
        let manager = WorkspaceManager::new();

        let result = manager.set_focus("".to_string()).await;

        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));
    }

    #[tokio::test]
    async fn test_focus_switch() {
        let manager = WorkspaceManager::new();
        let temp_dir = env::temp_dir();

        // Create two workspaces
        let summary_a = manager
            .create_workspace(temp_dir.to_str().unwrap())
            .await
            .unwrap();
        let summary_b = manager
            .create_workspace(temp_dir.to_str().unwrap())
            .await
            .unwrap();

        // Set focus to A
        manager
            .set_focus(summary_a.workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(manager.get_focus().await, Some(summary_a.workspace_id));

        // Switch focus to B
        manager
            .set_focus(summary_b.workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(manager.get_focus().await, Some(summary_b.workspace_id));
    }

    #[tokio::test]
    async fn test_get_workspace_found() {
        let manager = WorkspaceManager::new();
        let temp_dir = env::temp_dir();

        let summary = manager
            .create_workspace(temp_dir.to_str().unwrap())
            .await
            .unwrap();

        let result = manager.get_workspace(&summary.workspace_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_workspace_not_found() {
        let manager = WorkspaceManager::new();

        let result = manager
            .get_workspace(&"nonexistent-workspace-id".to_string())
            .await;

        assert!(matches!(
            result,
            Err(ApiError::WorkspaceNotFound { workspace_id }) if workspace_id == "nonexistent-workspace-id"
        ));
    }

    #[tokio::test]
    async fn test_create_agent() {
        let manager = WorkspaceManager::new();
        let temp_dir = env::temp_dir();

        // Create a workspace first
        let ws_summary = manager
            .create_workspace(temp_dir.to_str().unwrap())
            .await
            .unwrap();

        // Create an agent in the workspace
        let result = manager
            .create_agent(
                ws_summary.workspace_id.clone(),
                "claude-code".to_string(),
                Some("Test Agent".to_string()),
            )
            .await;

        assert!(result.is_ok());
        let agent_summary = result.unwrap();
        assert!(!agent_summary.agent_id.is_empty());
        assert_eq!(agent_summary.workspace_id, ws_summary.workspace_id);
        assert_eq!(agent_summary.plugin_id, "claude-code");
        assert_eq!(agent_summary.display_name, Some("Test Agent".to_string()));
    }

    #[tokio::test]
    async fn test_create_agent_unknown_workspace() {
        let manager = WorkspaceManager::new();

        let result = manager
            .create_agent(
                "nonexistent-workspace-id".to_string(),
                "claude-code".to_string(),
                None,
            )
            .await;

        assert!(matches!(
            result,
            Err(ApiError::WorkspaceNotFound { workspace_id }) if workspace_id == "nonexistent-workspace-id"
        ));
    }
}
