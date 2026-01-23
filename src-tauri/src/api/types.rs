//! Bridge types for commands and events.
//!
//! These types provide a stable API surface for the frontend,
//! hiding internal complexity.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Unique identifier for a workspace (UUID v4 string)
pub type WorkspaceId = String;

/// Unique identifier for an operation (UUID v4 string)
pub type OperationId = String;

/// Unique identifier for an agent (UUID v4 string)
pub type AgentId = String;

/// Unique identifier for an ACP session (UUID v4 string)
pub type SessionId = String;

/// Unique identifier for a terminal run (UUID v4 string)
pub type TerminalId = String;

/// Summary of a workspace returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    /// Unique workspace identifier
    pub workspace_id: WorkspaceId,
    /// Canonicalized absolute path to workspace root
    pub root_dir: String,
    /// Timestamp when workspace was created (milliseconds since epoch)
    /// Using f64 for JavaScript number compatibility
    pub created_at_ms: f64,
}

/// Summary of an agent returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentSummary {
    /// Unique agent identifier
    pub agent_id: AgentId,
    /// Workspace this agent belongs to
    pub workspace_id: WorkspaceId,
    /// Plugin identifier (e.g., "claude-code", "codex", "gemini")
    pub plugin_id: String,
    /// Optional display name for the agent
    pub display_name: Option<String>,
}

/// Plugin installation and update status returned to the frontend
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PluginStatus {
    /// Plugin identifier (e.g., "claude-code", "codex", "gemini")
    pub plugin_id: String,
    /// Whether the plugin is installed in the local cache
    pub installed: bool,
    /// Installed version string (if installed and version info available)
    pub installed_version: Option<String>,
    /// Latest available version (if update check was requested and succeeded)
    pub latest_version: Option<String>,
    /// Whether an update is available (if update check was requested)
    pub update_available: Option<bool>,
    /// Path to the plugin binary/entry point (if installed)
    pub bin_path: Option<String>,
}

// ============================================================================
// Permission System Types (US-04+)
// ============================================================================

/// Response when an async operation is started
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationStarted {
    /// Unique operation identifier for tracking
    pub operation_id: OperationId,
}

/// User decision for a permission request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PermissionDecision {
    /// Allow this operation once
    AllowOnce,
    /// Deny this operation
    Deny,
}

/// Source of a permission request
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum PermissionSource {
    /// User-initiated plugin installation
    InstallPlugin {
        plugin_id: String,
        version: Option<String>,
    },
    /// Agent-requested terminal execution
    TerminalRun { command: String },
    /// Agent-requested file read
    FsReadTextFile { path: String },
    /// Agent-requested file write
    FsWriteTextFile {
        path: String,
        content_preview: String,
        content_truncated: bool,
    },
}

/// Origin context for a permission request (optional scoping)
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOrigin {
    /// Workspace context (if applicable)
    pub workspace_id: Option<WorkspaceId>,
    /// Agent context (if applicable)
    pub agent_id: Option<String>,
    /// Session context (if applicable)
    pub session_id: Option<String>,
    /// Tool call context (if applicable)
    pub tool_call_id: Option<String>,
}

// ============================================================================
// Event Payloads
// ============================================================================

/// Event payload: permission requested (acp/permission_requested)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AcpPermissionRequestedEvent {
    /// Operation identifier for responding
    pub operation_id: OperationId,
    /// What is being requested
    pub source: PermissionSource,
    /// When the request was made (ms since epoch)
    pub requested_at_ms: f64,
    /// Optional origin context
    pub origin: Option<PermissionOrigin>,
}

/// Stream identifier for terminal output
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TerminalStream {
    Stdout,
    Stderr,
}

/// Event payload: terminal output chunk (terminal/output)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOutputEvent {
    /// Workspace this terminal belongs to
    pub workspace_id: WorkspaceId,
    /// Agent that initiated the terminal run
    pub agent_id: AgentId,
    /// Operation identifier for linking to permission/tool call
    pub operation_id: Option<OperationId>,
    /// Terminal identifier
    pub terminal_id: TerminalId,
    /// Output stream
    pub stream: TerminalStream,
    /// Output chunk (UTF-8)
    pub chunk: String,
}

/// Event payload: terminal exited (terminal/exited)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TerminalExitedEvent {
    /// Workspace this terminal belongs to
    pub workspace_id: WorkspaceId,
    /// Agent that initiated the terminal run
    pub agent_id: AgentId,
    /// Operation identifier for linking to permission/tool call
    pub operation_id: Option<OperationId>,
    /// Terminal identifier
    pub terminal_id: TerminalId,
    /// Exit code (None if unavailable)
    pub exit_code: Option<i32>,
    /// Whether the user explicitly stopped the process
    pub user_stopped: bool,
}

/// Event payload: plugin status changed (acp/plugin_status_changed)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AcpPluginStatusChangedEvent {
    /// Operation that triggered this change
    pub operation_id: OperationId,
    /// Current plugin status
    pub status: PluginStatus,
    /// Error message if operation failed
    pub error: Option<String>,
}

// ============================================================================
// Agent Runtime Types (US-06+)
// ============================================================================

/// Event payload: session update (acp/session_update)
///
/// US-07: Emitted when the ACP adapter sends session updates (message chunks,
/// tool calls, plans, etc.). The frontend subscribes to this event to display
/// streaming responses.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AcpSessionUpdateEvent {
    /// Workspace this agent belongs to
    pub workspace_id: WorkspaceId,
    /// Agent identifier
    pub agent_id: AgentId,
    /// Session identifier
    pub session_id: SessionId,
    /// Monotonic sequence number (per agent host) for deterministic ordering
    pub seq: u64,
    /// Timestamp when the host emitted this update (ms since UNIX epoch)
    pub emitted_at_ms: f64,
    /// The update payload
    pub update: AcpSessionUpdate,
}

/// ACP session update types
///
/// US-07: Represents different types of updates that can be received from
/// an ACP adapter during a session. Uses `serde_json::Value` for payload
/// flexibility (can be strongly typed later without breaking the contract).
///
/// Variants are inspired by Zed's SessionUpdate enum but kept flexible
/// for compatibility with various ACP adapters.
///
/// Field names match Claude Code ACP adapter format:
/// - agentMessageChunk: { content: [...] }
/// - availableCommandsUpdate: { availableCommands: [...] }
/// - toolCall: { toolCall: {...} } or inline fields
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AcpSessionUpdate {
    /// User message content chunk
    UserMessageChunk { content: serde_json::Value },
    /// Agent message content chunk
    AgentMessageChunk { content: serde_json::Value },
    /// Agent thought/reasoning chunk
    AgentThoughtChunk { content: serde_json::Value },
    /// Tool call initiated
    #[serde(rename_all = "camelCase")]
    ToolCall {
        #[serde(alias = "data", alias = "toolCall")]
        tool_call: serde_json::Value,
    },
    /// Tool call progress update
    #[serde(rename_all = "camelCase")]
    ToolCallUpdate {
        #[serde(alias = "data", alias = "toolCallUpdate")]
        tool_call_update: serde_json::Value,
    },
    /// Implementation plan
    Plan {
        #[serde(alias = "data")]
        plan: serde_json::Value,
    },
    /// Available commands update
    #[serde(rename_all = "camelCase")]
    AvailableCommandsUpdate {
        #[serde(alias = "data")]
        available_commands: serde_json::Value,
    },
    /// Current mode update
    #[serde(rename_all = "camelCase")]
    CurrentModeUpdate {
        #[serde(alias = "data", alias = "currentMode")]
        current_mode_id: serde_json::Value,
    },
    /// Configuration option update
    #[serde(rename_all = "camelCase")]
    ConfigOptionUpdate {
        #[serde(alias = "data")]
        config_options: serde_json::Value,
    },
    /// Turn completion signal with stop reason
    #[serde(rename_all = "camelCase")]
    TurnComplete {
        #[serde(alias = "stopReason")]
        stop_reason: serde_json::Value,
    },
    /// Raw/unknown update (fallback for unrecognized formats)
    Raw { json: serde_json::Value },
}

// ============================================================================
// Agent Runtime Types (US-06+) - continued
// ============================================================================

/// Acknowledgment returned when a prompt is sent
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SendPromptAck {
    /// Session identifier for tracking responses
    pub session_id: SessionId,
}

/// Runtime status of an agent
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum AgentRuntimeStatus {
    /// Agent is not running
    Stopped,
    /// Agent is starting up (spawn/initialize/new_session in progress)
    Starting,
    /// Agent is running with an active session
    Running {
        #[serde(rename = "sessionId")]
        session_id: SessionId,
    },
    /// Agent encountered an error
    Errored { message: String },
}

/// Event payload: agent status changed (agent/status_changed)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatusChangedEvent {
    /// Workspace this agent belongs to
    pub workspace_id: WorkspaceId,
    /// Agent identifier
    pub agent_id: AgentId,
    /// Current runtime status
    pub status: AgentRuntimeStatus,
}

/// API errors for frontend consumption
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ApiError {
    /// Invalid input parameter
    InvalidInput { message: String },
    /// Path does not exist or is not accessible
    PathNotFound { path: String },
    /// Path is not a directory
    PathNotDirectory { path: String },
    /// IO error during file system operation
    IoError { message: String },
    /// Workspace not found by ID
    WorkspaceNotFound {
        #[serde(rename = "workspaceId")]
        workspace_id: WorkspaceId,
    },
    /// Agent not found by ID
    AgentNotFound {
        #[serde(rename = "agentId")]
        agent_id: AgentId,
    },
    /// Operation not found by ID (e.g., permission already resolved, expired, or never existed)
    OperationNotFound {
        #[serde(rename = "operationId")]
        operation_id: OperationId,
    },
    /// Permission was denied by the user
    PermissionDenied {
        #[serde(rename = "operationId")]
        operation_id: OperationId,
    },
    /// Plugin installation is already in progress
    PluginInstallInProgress {
        #[serde(rename = "pluginId")]
        plugin_id: String,
    },
    /// Plugin is not installed (required for lazy startup)
    PluginNotInstalled {
        #[serde(rename = "pluginId")]
        plugin_id: String,
    },
    /// Plugin is installed but missing binary path in metadata
    PluginMissingBinPath {
        #[serde(rename = "pluginId")]
        plugin_id: String,
    },
    /// Protocol error during ACP communication
    ProtocolError { message: String },
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::InvalidInput { message } => write!(f, "Invalid input: {message}"),
            ApiError::PathNotFound { path } => write!(f, "Path not found: {path}"),
            ApiError::PathNotDirectory { path } => write!(f, "Path is not a directory: {path}"),
            ApiError::IoError { message } => write!(f, "IO error: {message}"),
            ApiError::WorkspaceNotFound { workspace_id } => {
                write!(f, "Workspace not found: {workspace_id}")
            }
            ApiError::AgentNotFound { agent_id } => {
                write!(f, "Agent not found: {agent_id}")
            }
            ApiError::OperationNotFound { operation_id } => {
                write!(f, "Operation not found: {operation_id}")
            }
            ApiError::PermissionDenied { operation_id } => {
                write!(f, "Permission denied: {operation_id}")
            }
            ApiError::PluginInstallInProgress { plugin_id } => {
                write!(f, "Plugin installation already in progress: {plugin_id}")
            }
            ApiError::PluginNotInstalled { plugin_id } => {
                write!(f, "Plugin not installed: {plugin_id}")
            }
            ApiError::PluginMissingBinPath { plugin_id } => {
                write!(f, "Plugin missing binary path: {plugin_id}")
            }
            ApiError::ProtocolError { message } => {
                write!(f, "Protocol error: {message}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acp_session_update_deserialize_agent_message_chunk() {
        let json = serde_json::json!({
            "type": "agentMessageChunk",
            "content": {"text": "Hello"}
        });

        let result: Result<AcpSessionUpdate, _> = serde_json::from_value(json);
        assert!(result.is_ok());
        matches!(result.unwrap(), AcpSessionUpdate::AgentMessageChunk { .. });
    }

    #[test]
    fn test_acp_session_update_deserialize_tool_call() {
        let json = serde_json::json!({
            "type": "toolCall",
            "data": {"id": "call-123", "name": "bash"}
        });

        let result: Result<AcpSessionUpdate, _> = serde_json::from_value(json);
        assert!(result.is_ok());
        matches!(result.unwrap(), AcpSessionUpdate::ToolCall { .. });
    }

    #[test]
    fn test_acp_session_update_deserialize_raw_fallback() {
        // Unknown type should deserialize as Raw
        let json = serde_json::json!({
            "type": "unknownType",
            "someField": "someValue"
        });

        let result: Result<AcpSessionUpdate, _> = serde_json::from_value(json.clone());
        // This will fail because unknownType is not a valid variant
        // The protocol layer wraps it as Raw manually when deserialization fails
        assert!(result.is_err());
    }

    #[test]
    fn test_acp_session_update_raw_variant() {
        let json = serde_json::json!({
            "type": "raw",
            "json": {"custom": "data"}
        });

        let result: Result<AcpSessionUpdate, _> = serde_json::from_value(json);
        assert!(result.is_ok());
        if let AcpSessionUpdate::Raw { json } = result.unwrap() {
            assert_eq!(json.get("custom").and_then(|v| v.as_str()), Some("data"));
        } else {
            panic!("Expected Raw variant");
        }
    }
}
