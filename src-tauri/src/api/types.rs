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
    // Future: TerminalRun, FsRead, FsWrite (US-08/10/11)
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
    /// Plugin installation is already in progress
    PluginInstallInProgress {
        #[serde(rename = "pluginId")]
        plugin_id: String,
    },
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
            ApiError::PluginInstallInProgress { plugin_id } => {
                write!(f, "Plugin installation already in progress: {plugin_id}")
            }
        }
    }
}
