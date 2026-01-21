//! Workspace management commands.
//!
//! Handles workspace creation, listing, and lifecycle management.

use std::sync::Arc;

use tauri::State;

use crate::api::types::{ApiError, WorkspaceId, WorkspaceSummary};
use crate::runtime::workspace_manager::WorkspaceManager;

async fn workspace_create_inner(
    workspace_manager: &WorkspaceManager,
    root_dir: String,
) -> Result<WorkspaceSummary, ApiError> {
    log::info!("workspace_create called with root_dir: {root_dir}");

    // Validate input is not empty
    if root_dir.trim().is_empty() {
        return Err(ApiError::InvalidInput {
            message: "Root directory cannot be empty".to_string(),
        });
    }

    // Delegate to manager
    workspace_manager.create_workspace(&root_dir).await
}

/// Creates a new workspace with the specified root directory.
///
/// # Arguments
/// * `root_dir` - Path to the workspace root directory (absolute or relative)
///
/// # Returns
/// * `WorkspaceSummary` - Summary of the created workspace including ID and canonicalized path
///
/// # Errors
/// * `ApiError::InvalidInput` - If root_dir is empty
/// * `ApiError::PathNotFound` - If the path does not exist
/// * `ApiError::PathNotDirectory` - If the path exists but is not a directory
/// * `ApiError::IoError` - If the path cannot be canonicalized
#[tauri::command]
#[specta::specta]
pub async fn workspace_create(
    workspace_manager: State<'_, Arc<WorkspaceManager>>,
    root_dir: String,
) -> Result<WorkspaceSummary, ApiError> {
    workspace_create_inner(&workspace_manager, root_dir).await
}

// --- Focus commands ---

async fn workspace_set_focus_inner(
    workspace_manager: &WorkspaceManager,
    workspace_id: WorkspaceId,
) -> Result<(), ApiError> {
    log::info!("workspace_set_focus called with workspace_id: {workspace_id}");
    workspace_manager.set_focus(workspace_id).await
}

/// Sets the currently focused workspace.
///
/// # Arguments
/// * `workspace_id` - ID of the workspace to focus
///
/// # Returns
/// * `()` - Focus was set successfully
///
/// # Errors
/// * `ApiError::InvalidInput` - If workspace_id is empty
/// * `ApiError::WorkspaceNotFound` - If the workspace does not exist
#[tauri::command]
#[specta::specta]
pub async fn workspace_set_focus(
    workspace_manager: State<'_, Arc<WorkspaceManager>>,
    workspace_id: WorkspaceId,
) -> Result<(), ApiError> {
    workspace_set_focus_inner(&workspace_manager, workspace_id).await
}

async fn workspace_get_focus_inner(
    workspace_manager: &WorkspaceManager,
) -> Result<Option<WorkspaceId>, ApiError> {
    log::debug!("workspace_get_focus called");
    Ok(workspace_manager.get_focus().await)
}

/// Gets the currently focused workspace ID.
///
/// # Returns
/// * `Some(WorkspaceId)` - ID of the focused workspace
/// * `None` - No workspace is currently focused
#[tauri::command]
#[specta::specta]
pub async fn workspace_get_focus(
    workspace_manager: State<'_, Arc<WorkspaceManager>>,
) -> Result<Option<WorkspaceId>, ApiError> {
    workspace_get_focus_inner(&workspace_manager).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workspace_create_invalid_input_empty() {
        let workspace_manager = WorkspaceManager::new();
        let result = workspace_create_inner(&workspace_manager, "".to_string()).await;
        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));
    }

    #[tokio::test]
    async fn test_workspace_create_invalid_input_whitespace() {
        let workspace_manager = WorkspaceManager::new();
        let result = workspace_create_inner(&workspace_manager, "   ".to_string()).await;
        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));
    }

    #[tokio::test]
    async fn test_workspace_get_focus_default_none() {
        let workspace_manager = WorkspaceManager::new();
        let result = workspace_get_focus_inner(&workspace_manager).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_workspace_set_focus_ok() {
        let workspace_manager = WorkspaceManager::new();
        let temp_dir = std::env::temp_dir();

        // Create a workspace first
        let summary =
            workspace_create_inner(&workspace_manager, temp_dir.to_str().unwrap().to_string())
                .await
                .unwrap();

        // Set focus
        let result =
            workspace_set_focus_inner(&workspace_manager, summary.workspace_id.clone()).await;
        assert!(result.is_ok());

        // Verify focus
        let focused = workspace_get_focus_inner(&workspace_manager).await.unwrap();
        assert_eq!(focused, Some(summary.workspace_id));
    }

    #[tokio::test]
    async fn test_workspace_set_focus_unknown_workspace() {
        let workspace_manager = WorkspaceManager::new();
        let result = workspace_set_focus_inner(&workspace_manager, "unknown-id".to_string()).await;
        assert!(matches!(result, Err(ApiError::WorkspaceNotFound { .. })));
    }
}
