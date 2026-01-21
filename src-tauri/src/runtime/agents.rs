//! Agent registry for managing agent entities within a workspace.
//!
//! This module provides the `AgentRegistry` which stores agent records
//! for a single workspace. Agents are created as entities first (not started),
//! and lazily started when the first prompt is sent (US-06).

use std::collections::HashMap;

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api::types::{AgentId, AgentSummary, ApiError, WorkspaceId};
use crate::plugins::manager::PluginManager;

/// Internal record for an agent entity (not yet started).
///
/// This represents the static configuration of an agent within a workspace.
/// The actual runtime state (process, session, etc.) is handled separately
/// in `AgentRuntime` (US-06+).
#[derive(Debug, Clone)]
pub struct AgentRecord {
    /// Unique identifier for this agent
    pub agent_id: AgentId,
    /// Plugin identifier (e.g., "claude-code", "codex", "gemini")
    pub plugin_id: String,
    /// Optional display name for the agent
    pub display_name: Option<String>,
}

impl AgentRecord {
    /// Convert to AgentSummary with workspace context.
    pub fn to_summary(&self, workspace_id: &WorkspaceId) -> AgentSummary {
        AgentSummary {
            agent_id: self.agent_id.clone(),
            workspace_id: workspace_id.clone(),
            plugin_id: self.plugin_id.clone(),
            display_name: self.display_name.clone(),
        }
    }
}

/// Registry of agent entities within a single workspace.
///
/// Thread-safe: Uses tokio::sync::Mutex for concurrent access.
/// Each workspace has its own AgentRegistry instance.
pub struct AgentRegistry {
    /// Map of agent ID to agent record
    agents: Mutex<HashMap<AgentId, AgentRecord>>,
}

impl AgentRegistry {
    /// Creates a new empty AgentRegistry.
    pub fn new() -> Self {
        Self {
            agents: Mutex::new(HashMap::new()),
        }
    }

    /// Creates a new agent entity in the registry.
    ///
    /// # Arguments
    /// * `plugin_id` - Plugin identifier (validated for format)
    /// * `display_name` - Optional display name (if Some, must be non-empty after trim)
    ///
    /// # Returns
    /// * `Ok(AgentRecord)` - The created agent record
    /// * `Err(ApiError::InvalidInput)` - If plugin_id or display_name is invalid
    pub async fn create_agent(
        &self,
        plugin_id: String,
        display_name: Option<String>,
    ) -> Result<AgentRecord, ApiError> {
        // Validate plugin_id format
        PluginManager::validate_plugin_id(&plugin_id)?;

        // Validate display_name if provided
        if let Some(ref name) = display_name {
            if name.trim().is_empty() {
                return Err(ApiError::InvalidInput {
                    message: "Display name cannot be empty".to_string(),
                });
            }
        }

        // Generate unique agent ID
        let agent_id = Uuid::new_v4().to_string();

        let record = AgentRecord {
            agent_id: agent_id.clone(),
            plugin_id,
            display_name,
        };

        // Insert into registry
        {
            let mut agents = self.agents.lock().await;
            agents.insert(agent_id.clone(), record.clone());
            log::info!(
                "Agent created: id={agent_id}, total_agents={}",
                agents.len()
            );
        }

        Ok(record)
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_agent_valid() {
        let registry = AgentRegistry::new();

        let result = registry
            .create_agent("claude-code".to_string(), Some("My Agent".to_string()))
            .await;

        assert!(result.is_ok());
        let record = result.unwrap();
        assert!(!record.agent_id.is_empty());
        assert_eq!(record.plugin_id, "claude-code");
        assert_eq!(record.display_name, Some("My Agent".to_string()));
    }

    #[tokio::test]
    async fn test_create_agent_none_display_name() {
        let registry = AgentRegistry::new();

        let result = registry.create_agent("codex".to_string(), None).await;

        assert!(result.is_ok());
        let record = result.unwrap();
        assert_eq!(record.plugin_id, "codex");
        assert_eq!(record.display_name, None);
    }

    #[tokio::test]
    async fn test_create_agent_invalid_plugin_id() {
        let registry = AgentRegistry::new();

        // Empty plugin_id
        let result = registry.create_agent("".to_string(), None).await;
        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));

        // Invalid characters
        let result = registry
            .create_agent("Invalid-Plugin".to_string(), None)
            .await;
        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));

        // Path traversal attempt
        let result = registry.create_agent("../evil".to_string(), None).await;
        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));
    }

    #[tokio::test]
    async fn test_create_agent_empty_display_name() {
        let registry = AgentRegistry::new();

        // Empty string
        let result = registry
            .create_agent("claude-code".to_string(), Some("".to_string()))
            .await;
        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));

        // Whitespace only
        let result = registry
            .create_agent("claude-code".to_string(), Some("   ".to_string()))
            .await;
        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));
    }

    #[tokio::test]
    async fn test_create_multiple_agents() {
        let registry = AgentRegistry::new();

        let record1 = registry
            .create_agent("claude-code".to_string(), Some("Agent 1".to_string()))
            .await
            .unwrap();

        let record2 = registry
            .create_agent("codex".to_string(), Some("Agent 2".to_string()))
            .await
            .unwrap();

        // Each agent should have a unique ID
        assert_ne!(record1.agent_id, record2.agent_id);
    }

    #[test]
    fn test_agent_record_to_summary() {
        let record = AgentRecord {
            agent_id: "test-agent-id".to_string(),
            plugin_id: "claude-code".to_string(),
            display_name: Some("Test Agent".to_string()),
        };

        let workspace_id = "test-workspace-id".to_string();
        let summary = record.to_summary(&workspace_id);

        assert_eq!(summary.agent_id, "test-agent-id");
        assert_eq!(summary.workspace_id, "test-workspace-id");
        assert_eq!(summary.plugin_id, "claude-code");
        assert_eq!(summary.display_name, Some("Test Agent".to_string()));
    }
}
