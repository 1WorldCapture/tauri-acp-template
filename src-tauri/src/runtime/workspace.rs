//! WorkspaceRuntime - container for a single workspace's state.
//!
//! Each workspace has its own runtime that will eventually hold:
//! - AgentRegistry (agents within this workspace)
//! - TerminalManager (terminals for this workspace)
//! - FsManager (file system operations scoped to this workspace)

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::types::{WorkspaceId, WorkspaceSummary};

/// Runtime state for a single workspace.
///
/// Thread-safe: This struct is designed to be wrapped in Arc<> and
/// shared across async tasks.
#[derive(Debug)]
pub struct WorkspaceRuntime {
    /// Unique identifier for this workspace
    workspace_id: WorkspaceId,
    /// Canonicalized absolute path to workspace root
    root_dir: PathBuf,
    /// Timestamp when workspace was created (milliseconds since epoch)
    created_at_ms: f64,
    // Future additions for subsequent user stories:
    // agent_registry: AgentRegistry,
    // terminal_manager: TerminalManager,
    // fs_manager: FsManager,
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

        Self {
            workspace_id,
            root_dir,
            created_at_ms,
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
}
