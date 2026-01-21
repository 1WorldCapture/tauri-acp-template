//! Workspace management commands.
//!
//! Handles workspace creation, listing, and lifecycle management.

use tauri::State;

use crate::api::types::{ApiError, WorkspaceSummary};
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
    workspace_manager: State<'_, WorkspaceManager>,
    root_dir: String,
) -> Result<WorkspaceSummary, ApiError> {
    workspace_create_inner(workspace_manager.inner(), root_dir).await
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
}
