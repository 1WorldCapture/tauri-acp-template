//! Bridge types for commands and events.
//!
//! These types provide a stable API surface for the frontend,
//! hiding internal complexity.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Unique identifier for a workspace (UUID v4 string)
pub type WorkspaceId = String;

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

/// API errors for frontend consumption
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type")]
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
    WorkspaceNotFound { workspace_id: WorkspaceId },
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
        }
    }
}
