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
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::InvalidInput { message } => write!(f, "Invalid input: {message}"),
            ApiError::PathNotFound { path } => write!(f, "Path not found: {path}"),
            ApiError::PathNotDirectory { path } => write!(f, "Path is not a directory: {path}"),
            ApiError::IoError { message } => write!(f, "IO error: {message}"),
        }
    }
}
