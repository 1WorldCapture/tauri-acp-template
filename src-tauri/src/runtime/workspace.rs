//! WorkspaceRuntime - container for a single workspace's state.
//!
//! Each workspace has its own runtime that holds:
//! - AgentRegistry (agents within this workspace)
//! - TerminalManager (terminals for this workspace)
//! - FsManager (file system operations scoped to this workspace)

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use std::sync::Arc;

use crate::api::types::{AgentId, AgentSummary, ApiError, WorkspaceId, WorkspaceSummary};
use crate::runtime::agents::{AgentRegistry, AgentRuntime};
use crate::runtime::fs::FsManager;
use crate::runtime::terminal::TerminalManager;

/// Runtime state for a single workspace.
///
/// Thread-safe: This struct is designed to be wrapped in Arc<> and
/// shared across async tasks.
pub struct WorkspaceRuntime {
    /// Unique identifier for this workspace
    workspace_id: WorkspaceId,
    /// Canonicalized absolute path to workspace root
    root_dir: PathBuf,
    /// Timestamp when workspace was created (milliseconds since epoch)
    created_at_ms: f64,
    /// Registry of agent entities within this workspace
    agent_registry: AgentRegistry,
    /// Terminal manager scoped to this workspace
    terminal_manager: Arc<TerminalManager>,
    /// File system manager scoped to this workspace
    fs_manager: Arc<FsManager>,
    // Future additions for subsequent user stories.
}

impl WorkspaceRuntime {
    /// Creates a new WorkspaceRuntime.
    ///
    /// # Arguments
    /// * `workspace_id` - Unique identifier (UUID v4 string)
    /// * `root_dir` - Canonicalized absolute path to workspace root
    pub fn new(workspace_id: WorkspaceId, root_dir: PathBuf) -> Self {
        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);

        log::info!(
            "Creating workspace runtime: id={workspace_id}, root={}",
            root_dir.display()
        );

        let terminal_manager = Arc::new(TerminalManager::new(root_dir.clone()));
        let fs_manager = Arc::new(FsManager::new(root_dir.clone()));

        Self {
            workspace_id,
            root_dir,
            created_at_ms,
            agent_registry: AgentRegistry::new(),
            terminal_manager,
            fs_manager,
        }
    }

    /// Returns a summary of this workspace for the frontend.
    pub fn summary(&self) -> WorkspaceSummary {
        WorkspaceSummary {
            workspace_id: self.workspace_id.clone(),
            root_dir: self.root_dir.display().to_string(),
            created_at_ms: self.created_at_ms,
        }
    }

    /// Creates an agent entity within this workspace.
    ///
    /// # Arguments
    /// * `plugin_id` - Plugin identifier (e.g., "claude-code", "codex", "gemini")
    /// * `display_name` - Optional display name for the agent
    ///
    /// # Returns
    /// * `Ok(AgentSummary)` - Summary of the created agent
    /// * `Err(ApiError)` - If validation fails
    pub async fn create_agent(
        &self,
        plugin_id: String,
        display_name: Option<String>,
    ) -> Result<AgentSummary, ApiError> {
        let record = self
            .agent_registry
            .create_agent(plugin_id, display_name)
            .await?;
        Ok(record.to_summary(&self.workspace_id))
    }

    /// Get or create an AgentRuntime for the given agent.
    ///
    /// This is called during lazy startup (US-06) when the first prompt is sent.
    /// The runtime is created if it doesn't exist.
    ///
    /// # Arguments
    /// * `agent_id` - The agent to get runtime for
    ///
    /// # Returns
    /// * `Ok(Arc<AgentRuntime>)` - The agent runtime
    /// * `Err(ApiError::AgentNotFound)` - If agent doesn't exist
    pub async fn ensure_agent_runtime(
        &self,
        agent_id: AgentId,
    ) -> Result<Arc<AgentRuntime>, ApiError> {
        self.agent_registry
            .ensure_runtime(self.workspace_id.clone(), agent_id)
            .await
    }

    /// Get the workspace ID.
    #[allow(dead_code)]
    pub fn workspace_id(&self) -> &WorkspaceId {
        &self.workspace_id
    }

    /// Get the workspace root directory.
    pub fn root_dir(&self) -> &PathBuf {
        &self.root_dir
    }

    /// Get the terminal manager for this workspace.
    pub fn terminal_manager(&self) -> Arc<TerminalManager> {
        self.terminal_manager.clone()
    }

    /// Get the file system manager for this workspace.
    pub fn fs_manager(&self) -> Arc<FsManager> {
        self.fs_manager.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_workspace_runtime_new() {
        let workspace_id = "test-workspace-123".to_string();
        let root_dir = env::temp_dir();

        let runtime = WorkspaceRuntime::new(workspace_id.clone(), root_dir.clone());

        assert_eq!(runtime.workspace_id, workspace_id);
        assert_eq!(runtime.root_dir, root_dir);
        assert!(runtime.created_at_ms > 0.0);
    }

    #[test]
    fn test_workspace_runtime_summary() {
        let workspace_id = "test-workspace-456".to_string();
        let root_dir = env::temp_dir();

        let runtime = WorkspaceRuntime::new(workspace_id.clone(), root_dir.clone());
        let summary = runtime.summary();

        assert_eq!(summary.workspace_id, workspace_id);
        assert_eq!(summary.root_dir, root_dir.display().to_string());
        assert!(summary.created_at_ms > 0.0);
    }

    #[tokio::test]
    async fn test_workspace_create_agent() {
        let workspace_id = "test-workspace-789".to_string();
        let root_dir = env::temp_dir();

        let runtime = WorkspaceRuntime::new(workspace_id.clone(), root_dir);

        let result = runtime
            .create_agent("claude-code".to_string(), Some("Test Agent".to_string()))
            .await;

        assert!(result.is_ok());
        let summary = result.unwrap();
        assert!(!summary.agent_id.is_empty());
        assert_eq!(summary.workspace_id, workspace_id);
        assert_eq!(summary.plugin_id, "claude-code");
        assert_eq!(summary.display_name, Some("Test Agent".to_string()));
    }
}
