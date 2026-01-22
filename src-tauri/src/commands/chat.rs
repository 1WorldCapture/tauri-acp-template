//! Chat commands for sending prompts and managing conversations.
//!
//! This module implements US-06 (lazy startup) and US-07 (prompt sending).
//! The `chat_send_prompt` command triggers agent lazy startup on first call
//! and sends the user's prompt to the agent.

use std::sync::Arc;

use tauri::{Manager, State};

use crate::api::types::{AgentId, ApiError, SendPromptAck, WorkspaceId};
use crate::plugins::manager::PluginManager;
use crate::runtime::permissions::PermissionHub;
use crate::runtime::workspace_manager::WorkspaceManager;

/// Inner function for testing without Tauri State wrapper.
async fn chat_send_prompt_inner(
    app: tauri::AppHandle,
    workspace_manager: &WorkspaceManager,
    plugin_manager: Arc<PluginManager>,
    workspace_id: WorkspaceId,
    agent_id: AgentId,
    prompt: String,
) -> Result<SendPromptAck, ApiError> {
    log::info!("chat_send_prompt: workspace={workspace_id}, agent={agent_id}");

    // Get workspace to access root_dir and agent registry
    let workspace = workspace_manager.get_workspace(&workspace_id).await?;
    let workspace_root = workspace.root_dir().clone();
    let terminal_manager = workspace.terminal_manager();
    let fs_manager = workspace.fs_manager();
    let permission_hub = app.state::<Arc<PermissionHub>>().inner().clone();

    // Ensure agent runtime exists (use workspace directly to avoid redundant lookup)
    let agent_runtime = workspace.ensure_agent_runtime(agent_id.clone()).await?;

    // Ensure agent is started (lazy startup on first prompt)
    let session_id = agent_runtime
        .ensure_started(
            app,
            workspace_root,
            plugin_manager,
            permission_hub,
            terminal_manager,
            fs_manager,
        )
        .await?;

    log::info!("Agent started: workspace={workspace_id}, agent={agent_id}, session={session_id}");

    // US-07: Send the prompt to the agent
    agent_runtime.send_prompt(prompt).await?;

    log::debug!("Prompt sent: workspace={workspace_id}, agent={agent_id}, session={session_id}");

    Ok(SendPromptAck { session_id })
}

/// Send a prompt to an agent, triggering lazy startup if needed.
///
/// US-06: This command triggers agent lazy startup on first call.
/// The agent is started (spawn/initialize/new_session) and session ID is returned.
///
/// US-07: Sends the user's prompt to the agent via JSON-RPC over stdin.
/// Streaming responses arrive asynchronously via `acp/session_update` events.
///
/// # Arguments
/// * `workspace_id` - ID of the workspace containing the agent
/// * `agent_id` - ID of the agent to send the prompt to
/// * `prompt` - The user's prompt text
///
/// # Returns
/// * `SendPromptAck` - Contains the session ID for tracking responses
///
/// # Events Emitted
/// * `agent/status_changed` - When agent starts (Starting → Running) or errors
/// * `acp/session_update` - Streaming session updates from the agent
///
/// # Errors
/// * `ApiError::WorkspaceNotFound` - If workspace doesn't exist
/// * `ApiError::AgentNotFound` - If agent doesn't exist in workspace
/// * `ApiError::PluginNotInstalled` - If the agent's plugin is not installed
/// * `ApiError::PluginMissingBinPath` - If plugin has no binary path
/// * `ApiError::ProtocolError` - If ACP communication fails
/// * `ApiError::IoError` - If writing to stdin fails
#[tauri::command]
#[specta::specta]
pub async fn chat_send_prompt(
    app: tauri::AppHandle,
    workspace_manager: State<'_, Arc<WorkspaceManager>>,
    plugin_manager: State<'_, Arc<PluginManager>>,
    workspace_id: WorkspaceId,
    agent_id: AgentId,
    prompt: String,
) -> Result<SendPromptAck, ApiError> {
    chat_send_prompt_inner(
        app,
        &workspace_manager,
        plugin_manager.inner().clone(),
        workspace_id,
        agent_id,
        prompt,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require a mock ACP adapter binary.
    // These tests validate the command logic up to the point of
    // agent connection.

    #[tokio::test]
    async fn test_ensure_agent_runtime_nonexistent_agent() {
        let workspace_manager = WorkspaceManager::new();

        // Create workspace
        let temp_dir = std::env::temp_dir();
        let ws_summary = workspace_manager
            .create_workspace(temp_dir.to_str().unwrap())
            .await
            .unwrap();

        // Try to get runtime for non-existent agent
        let result = workspace_manager
            .ensure_agent_runtime(ws_summary.workspace_id, "nonexistent-agent-id".to_string())
            .await;

        assert!(matches!(result, Err(ApiError::AgentNotFound { .. })));
    }

    #[tokio::test]
    async fn test_ensure_agent_runtime_nonexistent_workspace() {
        let workspace_manager = WorkspaceManager::new();

        // Try to get runtime for non-existent workspace
        let result = workspace_manager
            .ensure_agent_runtime(
                "nonexistent-workspace-id".to_string(),
                "some-agent-id".to_string(),
            )
            .await;

        assert!(matches!(result, Err(ApiError::WorkspaceNotFound { .. })));
    }
}
