//! Agent registry for managing agent entities within a workspace.
//!
//! This module provides:
//! - `AgentRegistry`: stores agent records for a single workspace
//! - `AgentRuntime`: manages the runtime state of a started agent (US-06+)
//!
//! Agents are created as entities first (not started), and lazily started
//! when the first prompt is sent (US-06).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api::types::{
    AgentId, AgentRuntimeStatus, AgentSummary, ApiError, SessionId, WorkspaceId,
};
use crate::plugins::manager::PluginManager;
use crate::protocols::acp::AcpAgent;
use crate::protocols::agent_connection::AgentConnection;
use crate::protocols::host::AgentHost;
use crate::runtime::agent_host::RuntimeAgentHost;
use crate::runtime::fs::FsManager;
use crate::runtime::permissions::PermissionHub;
use crate::runtime::terminal::TerminalManager;

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

// ============================================================================
// AgentRuntime (US-06+)
// ============================================================================

/// Runtime state of a started agent.
///
/// Created when an agent is lazily started (first prompt sent).
/// Holds the connection to the agent process and manages session state.
pub struct AgentRuntime {
    /// Agent identifier
    agent_id: AgentId,
    /// Workspace identifier
    workspace_id: WorkspaceId,
    /// Plugin identifier
    plugin_id: String,
    /// Current runtime status
    status: Mutex<AgentRuntimeStatus>,
    /// Active session ID (if running)
    session_id: Mutex<Option<SessionId>>,
    /// Protocol connection (if running)
    connection: Mutex<Option<Arc<dyn AgentConnection>>>,
    /// Lock to prevent concurrent startup attempts
    start_lock: Mutex<()>,
    /// App handle for emitting events (set during ensure_started)
    app: Mutex<Option<tauri::AppHandle>>,
}

impl AgentRuntime {
    /// Create a new AgentRuntime in Stopped state.
    pub fn new(agent_id: AgentId, workspace_id: WorkspaceId, plugin_id: String) -> Arc<Self> {
        Arc::new(Self {
            agent_id,
            workspace_id,
            plugin_id,
            status: Mutex::new(AgentRuntimeStatus::Stopped),
            session_id: Mutex::new(None),
            connection: Mutex::new(None),
            start_lock: Mutex::new(()),
            app: Mutex::new(None),
        })
    }

    /// Ensure the agent is started and return the session ID.
    ///
    /// This method is idempotent: if already started, returns the existing session ID.
    /// On first call, it:
    /// 1. Resolves the plugin binary via PluginManager
    /// 2. Spawns the agent process via AcpAgent
    /// 3. Initializes the connection and creates a session
    /// 4. Emits `agent/status_changed` events
    ///
    /// # Arguments
    /// * `app` - Tauri application handle for events
    /// * `workspace_root` - Root directory of the workspace
    /// * `plugin_manager` - For resolving the plugin binary
    /// * `permission_hub` - Permission hub for approval flow
    /// * `terminal_manager` - Terminal manager scoped to the workspace
    /// * `fs_manager` - File system manager scoped to the workspace
    ///
    /// # Returns
    /// * `Ok(SessionId)` - The session ID (existing or newly created)
    /// * `Err(ApiError)` - Plugin not installed, spawn failed, etc.
    pub async fn ensure_started(
        self: &Arc<Self>,
        app: tauri::AppHandle,
        workspace_root: PathBuf,
        plugin_manager: Arc<PluginManager>,
        permission_hub: Arc<PermissionHub>,
        terminal_manager: Arc<TerminalManager>,
        fs_manager: Arc<FsManager>,
    ) -> Result<SessionId, ApiError> {
        // Fast path: already running
        {
            let session_id_guard = self.session_id.lock().await;
            if let Some(ref session_id) = *session_id_guard {
                log::debug!(
                    "Agent already started: agent={}, session={}",
                    self.agent_id,
                    session_id
                );
                return Ok(session_id.clone());
            }
        }

        // Acquire start lock to prevent concurrent startup
        let _start_guard = self.start_lock.lock().await;

        // Double-check after acquiring lock
        {
            let session_id_guard = self.session_id.lock().await;
            if let Some(ref session_id) = *session_id_guard {
                log::debug!(
                    "Agent started by another task: agent={}, session={}",
                    self.agent_id,
                    session_id
                );
                return Ok(session_id.clone());
            }
        }

        // Update status to Starting
        {
            let mut status = self.status.lock().await;
            *status = AgentRuntimeStatus::Starting;
        }

        // Create host for callbacks
        let host = RuntimeAgentHost::new(
            app.clone(),
            self.workspace_id.clone(),
            self.agent_id.clone(),
            permission_hub,
            terminal_manager,
            fs_manager,
        );

        // Emit Starting status
        host.set_status(AgentRuntimeStatus::Starting);

        // Resolve plugin binary
        let plugin_command = match plugin_manager.resolve_bin(self.plugin_id.clone()).await {
            Ok(cmd) => cmd,
            Err(e) => {
                log::error!(
                    "Failed to resolve plugin binary: agent={}, plugin={}, error={}",
                    self.agent_id,
                    self.plugin_id,
                    e
                );
                let error_status = AgentRuntimeStatus::Errored {
                    message: e.to_string(),
                };
                *self.status.lock().await = error_status.clone();
                host.set_status(error_status);
                return Err(e);
            }
        };

        log::info!(
            "Starting agent: agent={}, plugin={}, bin={:?}",
            self.agent_id,
            self.plugin_id,
            plugin_command.path
        );

        // Connect via ACP
        let (connection, session_id) =
            match AcpAgent::connect(plugin_command, workspace_root, host.clone()).await {
                Ok(result) => result,
                Err(e) => {
                    log::error!(
                        "Failed to connect to agent: agent={}, error={}",
                        self.agent_id,
                        e
                    );
                    let error_status = AgentRuntimeStatus::Errored {
                        message: e.to_string(),
                    };
                    *self.status.lock().await = error_status.clone();
                    host.set_status(error_status);
                    return Err(e);
                }
            };

        // Store connection, session, and app handle
        {
            let mut conn_guard = self.connection.lock().await;
            *conn_guard = Some(connection);
        }
        {
            let mut session_guard = self.session_id.lock().await;
            *session_guard = Some(session_id.clone());
        }
        {
            let mut app_guard = self.app.lock().await;
            *app_guard = Some(app);
        }

        // Update status to Running
        let running_status = AgentRuntimeStatus::Running {
            session_id: session_id.clone(),
        };
        {
            let mut status = self.status.lock().await;
            *status = running_status.clone();
        }
        host.set_status(running_status);

        log::info!(
            "Agent started: agent={}, session={}",
            self.agent_id,
            session_id
        );

        Ok(session_id)
    }

    /// Send a prompt to the running agent.
    ///
    /// US-07: Sends the user's prompt text to the agent via the protocol connection.
    /// The agent must already be started (call ensure_started first).
    /// Streaming responses will arrive asynchronously via AgentHost callbacks.
    ///
    /// # Arguments
    /// * `prompt` - The user's prompt text
    ///
    /// # Returns
    /// * `Ok(())` - Prompt sent successfully
    /// * `Err(ApiError::ProtocolError)` - If agent is not running or connection unavailable
    /// * `Err(ApiError::IoError)` - If writing to the protocol fails
    pub async fn send_prompt(self: &Arc<Self>, prompt: String) -> Result<(), ApiError> {
        // Get session_id (fail if agent not running)
        let session_id = {
            let session_guard = self.session_id.lock().await;
            session_guard
                .clone()
                .ok_or_else(|| ApiError::ProtocolError {
                    message: "Agent not running".to_string(),
                })?
        };

        // Get connection (fail if connection unavailable)
        let connection = {
            let conn_guard = self.connection.lock().await;
            conn_guard.clone().ok_or_else(|| ApiError::ProtocolError {
                message: "Agent connection not available".to_string(),
            })?
        };

        // Call the trait method to send prompt
        connection.send_prompt(session_id, prompt).await
    }

    /// Stop the current turn for the given session.
    ///
    /// US-12: Cancels the active turn via the protocol connection.
    /// The agent must already be started (call ensure_started first).
    ///
    /// # Arguments
    /// * `session_id` - The session to cancel the current turn for
    ///
    /// # Returns
    /// * `Ok(())` - Cancel request sent successfully
    /// * `Err(ApiError::ProtocolError)` - If agent is not running or connection unavailable
    /// * `Err(ApiError::IoError)` - If writing to the protocol fails
    pub async fn stop_turn(self: &Arc<Self>, session_id: SessionId) -> Result<(), ApiError> {
        // Ensure agent is running and the session matches
        let current_session_id = {
            let session_guard = self.session_id.lock().await;
            session_guard
                .clone()
                .ok_or_else(|| ApiError::ProtocolError {
                    message: "Agent not running".to_string(),
                })?
        };

        if current_session_id != session_id {
            return Err(ApiError::InvalidInput {
                message: "Session ID does not match active session".to_string(),
            });
        }

        // Get connection (fail if connection unavailable)
        let connection = {
            let conn_guard = self.connection.lock().await;
            conn_guard.clone().ok_or_else(|| ApiError::ProtocolError {
                message: "Agent connection not available".to_string(),
            })?
        };

        // Call the trait method to cancel turn
        connection.cancel_turn(session_id).await
    }
}

/// Registry of agent entities within a single workspace.
///
/// Thread-safe: Uses tokio::sync::Mutex for concurrent access.
/// Each workspace has its own AgentRegistry instance.
pub struct AgentRegistry {
    /// Map of agent ID to agent record
    agents: Mutex<HashMap<AgentId, AgentRecord>>,
    /// Map of agent ID to agent runtime (lazily created on first prompt)
    runtimes: Mutex<HashMap<AgentId, Arc<AgentRuntime>>>,
}

impl AgentRegistry {
    /// Creates a new empty AgentRegistry.
    pub fn new() -> Self {
        Self {
            agents: Mutex::new(HashMap::new()),
            runtimes: Mutex::new(HashMap::new()),
        }
    }

    /// Lists all agents in the registry.
    ///
    /// # Returns
    /// * `Vec<AgentRecord>` - List of all agent records
    pub async fn list_agents(&self) -> Vec<AgentRecord> {
        let agents = self.agents.lock().await;
        agents.values().cloned().collect()
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

    /// Get an agent record by ID.
    ///
    /// # Arguments
    /// * `agent_id` - The agent to look up
    ///
    /// # Returns
    /// * `Ok(AgentRecord)` - The agent record
    /// * `Err(ApiError::AgentNotFound)` - If agent doesn't exist
    pub async fn get_agent(&self, agent_id: &AgentId) -> Result<AgentRecord, ApiError> {
        let agents = self.agents.lock().await;
        agents
            .get(agent_id)
            .cloned()
            .ok_or_else(|| ApiError::AgentNotFound {
                agent_id: agent_id.clone(),
            })
    }

    /// Get or create an AgentRuntime for the given agent.
    ///
    /// This is called during lazy startup to get the runtime handle.
    /// The runtime is created if it doesn't exist.
    ///
    /// # Arguments
    /// * `workspace_id` - The workspace this agent belongs to
    /// * `agent_id` - The agent to get runtime for
    ///
    /// # Returns
    /// * `Ok(Arc<AgentRuntime>)` - The agent runtime (existing or newly created)
    /// * `Err(ApiError::AgentNotFound)` - If agent doesn't exist in registry
    pub async fn ensure_runtime(
        &self,
        workspace_id: WorkspaceId,
        agent_id: AgentId,
    ) -> Result<Arc<AgentRuntime>, ApiError> {
        // Verify agent exists
        let record = self.get_agent(&agent_id).await?;

        // Check if runtime already exists
        {
            let runtimes = self.runtimes.lock().await;
            if let Some(runtime) = runtimes.get(&agent_id) {
                return Ok(runtime.clone());
            }
        }

        // Create new runtime
        let runtime = AgentRuntime::new(agent_id.clone(), workspace_id, record.plugin_id);

        // Insert into runtimes map
        {
            let mut runtimes = self.runtimes.lock().await;
            // Double-check in case another task created it
            if let Some(existing) = runtimes.get(&agent_id) {
                return Ok(existing.clone());
            }
            runtimes.insert(agent_id.clone(), runtime.clone());
            log::debug!(
                "Created agent runtime: agent={}, total_runtimes={}",
                agent_id,
                runtimes.len()
            );
        }

        Ok(runtime)
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
    use async_trait::async_trait;
    use tokio::sync::Mutex as TokioMutex;

    struct MockConnection {
        canceled_session: Arc<TokioMutex<Option<SessionId>>>,
    }

    #[async_trait]
    impl AgentConnection for MockConnection {
        async fn send_prompt(
            &self,
            _session_id: SessionId,
            _prompt: String,
        ) -> Result<(), ApiError> {
            Ok(())
        }

        async fn cancel_turn(&self, session_id: SessionId) -> Result<(), ApiError> {
            let mut guard = self.canceled_session.lock().await;
            *guard = Some(session_id);
            Ok(())
        }

        async fn shutdown(&self) -> Result<(), ApiError> {
            Ok(())
        }
    }

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

    #[tokio::test]
    async fn test_agent_runtime_stop_turn_not_running() {
        let runtime = AgentRuntime::new(
            "agent-123".to_string(),
            "workspace-123".to_string(),
            "claude-code".to_string(),
        );

        let result = runtime.stop_turn("session-123".to_string()).await;
        assert!(matches!(result, Err(ApiError::ProtocolError { .. })));
    }

    #[tokio::test]
    async fn test_agent_runtime_stop_turn_calls_cancel() {
        let runtime = AgentRuntime::new(
            "agent-123".to_string(),
            "workspace-123".to_string(),
            "claude-code".to_string(),
        );

        let canceled_session = Arc::new(TokioMutex::new(None));
        let connection = Arc::new(MockConnection {
            canceled_session: canceled_session.clone(),
        });

        {
            let mut session_guard = runtime.session_id.lock().await;
            *session_guard = Some("session-123".to_string());
        }
        {
            let mut conn_guard = runtime.connection.lock().await;
            *conn_guard = Some(connection);
        }

        let result = runtime.stop_turn("session-123".to_string()).await;
        assert!(result.is_ok());

        let canceled = canceled_session.lock().await;
        assert_eq!(canceled.as_deref(), Some("session-123"));
    }

    #[tokio::test]
    async fn test_agent_runtime_stop_turn_session_mismatch() {
        let runtime = AgentRuntime::new(
            "agent-123".to_string(),
            "workspace-123".to_string(),
            "claude-code".to_string(),
        );

        let canceled_session = Arc::new(TokioMutex::new(None));
        let connection = Arc::new(MockConnection {
            canceled_session: canceled_session.clone(),
        });

        {
            let mut session_guard = runtime.session_id.lock().await;
            *session_guard = Some("session-123".to_string());
        }
        {
            let mut conn_guard = runtime.connection.lock().await;
            *conn_guard = Some(connection);
        }

        let result = runtime.stop_turn("session-999".to_string()).await;
        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));

        let canceled = canceled_session.lock().await;
        assert!(canceled.is_none());
    }
}
