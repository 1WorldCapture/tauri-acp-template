//! Path utilities for workspace boundary validation.
//!
//! MVP: Covers root canonicalization. Future: symlink/.. security checks.

use std::io::ErrorKind;
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

/// Resolve a file path within a workspace root.
///
/// This performs a canonicalization-based boundary check to ensure
/// the resolved path stays within the workspace root. The input path
/// must exist to be canonicalized.
pub fn resolve_path_in_workspace(root: &Path, input: &str) -> Result<PathBuf, ApiError> {
    if input.trim().is_empty() {
        return Err(ApiError::InvalidInput {
            message: "Path cannot be empty".to_string(),
        });
    }

    let root_display = root.display().to_string();
    let root = root.canonicalize().map_err(|e| ApiError::IoError {
        message: format!("Failed to canonicalize workspace root '{root_display}': {e}"),
    })?;

    let input_path = Path::new(input);
    if input_path.is_absolute() {
        let resolved = input_path.canonicalize().map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                if !input_path.starts_with(&root) {
                    ApiError::InvalidInput {
                        message: format!("Path escapes workspace root: {input}"),
                    }
                } else {
                    ApiError::PathNotFound {
                        path: input.to_string(),
                    }
                }
            } else {
                ApiError::IoError {
                    message: format!("Failed to canonicalize path '{input}': {e}"),
                }
            }
        })?;

        if !resolved.starts_with(&root) {
            return Err(ApiError::InvalidInput {
                message: format!("Path escapes workspace root: {input}"),
            });
        }

        return Ok(resolved);
    }

    let candidate = root.join(input_path);

    let resolved = candidate.canonicalize().map_err(|e| {
        if e.kind() == ErrorKind::NotFound {
            ApiError::PathNotFound {
                path: input.to_string(),
            }
        } else {
            ApiError::IoError {
                message: format!("Failed to canonicalize path '{input}': {e}"),
            }
        }
    })?;

    if !resolved.starts_with(root) {
        return Err(ApiError::InvalidInput {
            message: format!("Path escapes workspace root: {input}"),
        });
    }

    Ok(resolved)
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

    #[test]
    fn test_resolve_path_in_workspace_relative() {
        let root = env::temp_dir().join(format!("ws_root_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("failed to create root dir");

        let file_path = root.join("test.txt");
        std::fs::write(&file_path, "hello").expect("failed to write file");

        let resolved = resolve_path_in_workspace(&root, "test.txt").unwrap();
        assert!(resolved.is_absolute());
        assert_eq!(resolved, file_path.canonicalize().unwrap());

        std::fs::remove_file(&file_path).expect("failed to remove file");
        std::fs::remove_dir_all(&root).expect("failed to remove root dir");
    }

    #[test]
    fn test_resolve_path_in_workspace_absolute_inside_root() {
        let root = env::temp_dir().join(format!("ws_root_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("failed to create root dir");

        let file_path = root.join("abs.txt");
        std::fs::write(&file_path, "abs").expect("failed to write file");

        let resolved = resolve_path_in_workspace(&root, file_path.to_str().unwrap()).unwrap();
        assert_eq!(resolved, file_path.canonicalize().unwrap());

        std::fs::remove_file(&file_path).expect("failed to remove file");
        std::fs::remove_dir_all(&root).expect("failed to remove root dir");
    }

    #[test]
    fn test_resolve_path_in_workspace_absolute_outside_root() {
        let root = env::temp_dir().join(format!("ws_root_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("failed to create root dir");

        let outside_dir = env::temp_dir().join(format!("outside_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&outside_dir).expect("failed to create outside dir");
        let outside_file = outside_dir.join("secret.txt");
        std::fs::write(&outside_file, "secret").expect("failed to write outside file");

        let result = resolve_path_in_workspace(&root, outside_file.to_str().unwrap());
        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));

        std::fs::remove_file(&outside_file).expect("failed to remove outside file");
        std::fs::remove_dir_all(&outside_dir).expect("failed to remove outside dir");
        std::fs::remove_dir_all(&root).expect("failed to remove root dir");
    }

    #[test]
    fn test_resolve_path_in_workspace_rejects_escape() {
        let root = env::temp_dir().join(format!("ws_root_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("failed to create root dir");

        let outside_dir = env::temp_dir().join(format!("outside_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&outside_dir).expect("failed to create outside dir");
        let outside_file = outside_dir.join("secret.txt");
        std::fs::write(&outside_file, "secret").expect("failed to write outside file");

        let escape_path = format!(
            "../{}/secret.txt",
            outside_dir.file_name().unwrap().to_string_lossy()
        );
        let result = resolve_path_in_workspace(&root, &escape_path);
        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));

        std::fs::remove_file(&outside_file).expect("failed to remove outside file");
        std::fs::remove_dir_all(&outside_dir).expect("failed to remove outside dir");
        std::fs::remove_dir_all(&root).expect("failed to remove root dir");
    }
}
