//! Agent management commands.
//!
//! Handles agent creation and lifecycle management within workspaces.

use std::sync::Arc;

use tauri::State;

use crate::api::types::{AgentSummary, ApiError, WorkspaceId};
use crate::runtime::workspace_manager::WorkspaceManager;

async fn agent_create_inner(
    workspace_manager: &WorkspaceManager,
    workspace_id: WorkspaceId,
    plugin_id: String,
    display_name: Option<String>,
) -> Result<AgentSummary, ApiError> {
    log::info!("agent_create: workspace={workspace_id}, plugin={plugin_id}");

    // Validate workspace_id is not empty
    if workspace_id.trim().is_empty() {
        return Err(ApiError::InvalidInput {
            message: "Workspace ID cannot be empty".to_string(),
        });
    }

    workspace_manager
        .create_agent(workspace_id, plugin_id, display_name)
        .await
}

/// Creates a new agent entity within a workspace.
///
/// # Arguments
/// * `workspace_id` - ID of the workspace to create the agent in
/// * `plugin_id` - Plugin identifier (e.g., "claude-code", "codex", "gemini")
/// * `display_name` - Optional display name for the agent
///
/// # Returns
/// * `AgentSummary` - Summary of the created agent including ID
///
/// # Errors
/// * `ApiError::WorkspaceNotFound` - If the workspace does not exist
/// * `ApiError::InvalidInput` - If plugin_id or display_name is invalid
#[tauri::command]
#[specta::specta]
pub async fn agent_create(
    workspace_manager: State<'_, Arc<WorkspaceManager>>,
    workspace_id: WorkspaceId,
    plugin_id: String,
    display_name: Option<String>,
) -> Result<AgentSummary, ApiError> {
    agent_create_inner(&workspace_manager, workspace_id, plugin_id, display_name).await
}

async fn agent_list_inner(
    workspace_manager: &WorkspaceManager,
    workspace_id: WorkspaceId,
) -> Result<Vec<AgentSummary>, ApiError> {
    log::info!("agent_list: workspace={workspace_id}");

    // Validate workspace_id is not empty
    if workspace_id.trim().is_empty() {
        return Err(ApiError::InvalidInput {
            message: "Workspace ID cannot be empty".to_string(),
        });
    }

    workspace_manager.list_agents(workspace_id).await
}

/// Lists all agents within a workspace.
///
/// # Arguments
/// * `workspace_id` - ID of the workspace to list agents from
///
/// # Returns
/// * `Vec<AgentSummary>` - List of all agent summaries
///
/// # Errors
/// * `ApiError::WorkspaceNotFound` - If the workspace does not exist
#[tauri::command]
#[specta::specta]
pub async fn agent_list(
    workspace_manager: State<'_, Arc<WorkspaceManager>>,
    workspace_id: WorkspaceId,
) -> Result<Vec<AgentSummary>, ApiError> {
    agent_list_inner(&workspace_manager, workspace_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_create_valid() {
        let workspace_manager = WorkspaceManager::new();
        let temp_dir = std::env::temp_dir();

        // Create a workspace first
        let ws_summary = workspace_manager
            .create_workspace(temp_dir.to_str().unwrap())
            .await
            .unwrap();

        // Create an agent
        let result = agent_create_inner(
            &workspace_manager,
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
    }

    #[tokio::test]
    async fn test_agent_create_invalid_plugin_id() {
        let workspace_manager = WorkspaceManager::new();
        let temp_dir = std::env::temp_dir();

        // Create a workspace first
        let ws_summary = workspace_manager
            .create_workspace(temp_dir.to_str().unwrap())
            .await
            .unwrap();

        // Try to create agent with invalid plugin_id
        let result = agent_create_inner(
            &workspace_manager,
            ws_summary.workspace_id,
            "Invalid-Plugin".to_string(), // uppercase not allowed
            None,
        )
        .await;

        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));
    }

    #[tokio::test]
    async fn test_agent_create_unknown_workspace() {
        let workspace_manager = WorkspaceManager::new();

        let result = agent_create_inner(
            &workspace_manager,
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
