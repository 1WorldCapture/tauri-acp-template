//! Path utilities for workspace boundary validation.
//!
//! MVP: Covers root canonicalization. Future: symlink/.. security checks.

use std::path::{Path, PathBuf};

use crate::api::types::ApiError;

/// Canonicalizes and validates a workspace root directory.
///
/// # Arguments
/// * `root` - The path to validate (can be relative or absolute)
///
/// # Returns
/// * `Ok(PathBuf)` - Canonicalized absolute path
/// * `Err(ApiError)` - If path doesn't exist, isn't a directory, or can't be canonicalized
pub fn canonicalize_workspace_root(root: &str) -> Result<PathBuf, ApiError> {
    if root.trim().is_empty() {
        return Err(ApiError::InvalidInput {
            message: "Root directory cannot be empty".to_string(),
        });
    }

    let path = Path::new(root);

    // Check if path exists
    if !path.exists() {
        return Err(ApiError::PathNotFound {
            path: root.to_string(),
        });
    }

    // Check if path is a directory
    if !path.is_dir() {
        return Err(ApiError::PathNotDirectory {
            path: root.to_string(),
        });
    }

    // Canonicalize the path (resolves symlinks, removes . and .., makes absolute)
    path.canonicalize().map_err(|e| ApiError::IoError {
        message: format!("Failed to canonicalize path '{root}': {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use uuid::Uuid;

    #[test]
    fn test_canonicalize_existing_directory() {
        // Use temp dir which always exists
        let temp_dir = env::temp_dir();
        let result = canonicalize_workspace_root(temp_dir.to_str().unwrap());
        assert!(result.is_ok());
        assert!(result.unwrap().is_absolute());
    }

    #[test]
    fn test_canonicalize_nonexistent_path() {
        let result = canonicalize_workspace_root("/nonexistent/path/12345");
        assert!(matches!(result, Err(ApiError::PathNotFound { .. })));
    }

    #[test]
    fn test_canonicalize_empty_path() {
        let result = canonicalize_workspace_root("");
        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));
    }

    #[test]
    fn test_canonicalize_file_not_directory() {
        // Create a temp file to test with
        let temp_file = env::temp_dir().join(format!("test_file_not_dir_{}.txt", Uuid::new_v4()));
        std::fs::write(&temp_file, "test").expect("failed to create temp file");
        let result = canonicalize_workspace_root(temp_file.to_str().unwrap());
        assert!(matches!(result, Err(ApiError::PathNotDirectory { .. })));
        std::fs::remove_file(&temp_file).expect("failed to remove temp file");
    }
}
