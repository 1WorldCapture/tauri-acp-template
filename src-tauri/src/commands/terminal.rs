//! Terminal-related commands.
//!
//! US-09: Allow the user to stop a running terminal command.

use std::sync::Arc;

use tauri::State;

use crate::api::types::{ApiError, TerminalId, WorkspaceId};
use crate::runtime::workspace_manager::WorkspaceManager;

/// Stop a running terminal command by ID.
///
/// # Arguments
/// * `workspace_id` - ID of the workspace containing the terminal
/// * `terminal_id` - ID of the terminal run to stop
///
/// # Returns
/// * `Ok(())` - Stop request accepted (idempotent)
/// * `Err(ApiError)` - If workspace is missing or inputs are invalid
#[tauri::command]
#[specta::specta]
pub async fn terminal_kill(
    workspace_manager: State<'_, Arc<WorkspaceManager>>,
    workspace_id: WorkspaceId,
    terminal_id: TerminalId,
) -> Result<(), ApiError> {
    log::info!("terminal_kill: workspace={workspace_id}, terminal={terminal_id}");

    if workspace_id.trim().is_empty() {
        return Err(ApiError::InvalidInput {
            message: "Workspace ID cannot be empty".to_string(),
        });
    }

    let workspace = workspace_manager.get_workspace(&workspace_id).await?;
    let terminal_manager = workspace.terminal_manager();
    terminal_manager.kill(terminal_id).await
}
