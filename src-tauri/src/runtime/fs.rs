//! FsManager - reads files scoped to a workspace root.
//!
//! US-10: Provides read_text_file with workspace boundary validation.

use std::path::PathBuf;

use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::api::types::ApiError;
use crate::runtime::path::{resolve_path_in_workspace, resolve_write_target_in_workspace};

const MAX_READ_BYTES: u64 = 1024 * 1024;

/// Per-workspace file system manager.
pub struct FsManager {
    workspace_root: PathBuf,
}

impl FsManager {
    /// Create a new FsManager scoped to a workspace root.
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    /// Read a text file within the workspace boundary.
    pub async fn read_text_file(&self, path: String) -> Result<String, ApiError> {
        let resolved = resolve_path_in_workspace(&self.workspace_root, &path)?;

        let metadata = fs::metadata(&resolved)
            .await
            .map_err(|e| ApiError::IoError {
                message: format!("Failed to read metadata for '{path}': {e}"),
            })?;

        if !metadata.is_file() {
            return Err(ApiError::InvalidInput {
                message: format!("Path is not a file: {path}"),
            });
        }

        if metadata.len() > MAX_READ_BYTES {
            return Err(ApiError::InvalidInput {
                message: format!("File too large to read: {path} (max {MAX_READ_BYTES} bytes)"),
            });
        }

        fs::read_to_string(&resolved)
            .await
            .map_err(|e| ApiError::IoError {
                message: format!("Failed to read file '{path}': {e}"),
            })
    }

    /// Write a text file within the workspace boundary.
    pub async fn write_text_file(&self, path: String, content: String) -> Result<u64, ApiError> {
        let resolved = resolve_write_target_in_workspace(&self.workspace_root, &path)?;

        let parent = resolved.parent().ok_or_else(|| ApiError::InvalidInput {
            message: format!("Path must include a parent directory: {path}"),
        })?;

        let parent_metadata = fs::metadata(&parent).await.map_err(|e| ApiError::IoError {
            message: format!("Failed to read metadata for '{path}': {e}"),
        })?;

        if !parent_metadata.is_dir() {
            return Err(ApiError::InvalidInput {
                message: format!("Parent is not a directory: {path}"),
            });
        }

        match fs::symlink_metadata(&resolved).await {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(ApiError::InvalidInput {
                        message: format!("Path is a symlink: {path}"),
                    });
                }
                if metadata.is_dir() {
                    return Err(ApiError::InvalidInput {
                        message: format!("Path is a directory: {path}"),
                    });
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(ApiError::IoError {
                    message: format!("Failed to read metadata for '{path}': {e}"),
                });
            }
        }

        let temp_path = parent.join(format!(".tmp_write_{}", Uuid::new_v4()));
        let mut temp_file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .await
            .map_err(|e| ApiError::IoError {
                message: format!("Failed to create temp file for '{path}': {e}"),
            })?;

        temp_file
            .write_all(content.as_bytes())
            .await
            .map_err(|e| ApiError::IoError {
                message: format!("Failed to write temp file for '{path}': {e}"),
            })?;

        temp_file.flush().await.map_err(|e| ApiError::IoError {
            message: format!("Failed to flush temp file for '{path}': {e}"),
        })?;

        temp_file.sync_all().await.map_err(|e| ApiError::IoError {
            message: format!("Failed to sync temp file for '{path}': {e}"),
        })?;

        drop(temp_file);

        if let Err(e) = replace_file(&temp_path, &resolved).await {
            let _ = fs::remove_file(&temp_path).await;
            return Err(e);
        }

        Ok(content.len() as u64)
    }
}

async fn replace_file(from: &PathBuf, to: &PathBuf) -> Result<(), ApiError> {
    match fs::rename(from, to).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            fs::remove_file(to).await.map_err(|err| ApiError::IoError {
                message: format!("Failed to replace existing file: {err}"),
            })?;
            fs::rename(from, to).await.map_err(|err| ApiError::IoError {
                message: format!("Failed to replace file: {err}"),
            })
        }
        Err(e) => Err(ApiError::IoError {
            message: format!("Failed to replace file: {e}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_read_text_file_success() {
        let root = env::temp_dir().join(format!("fs_root_{}", Uuid::new_v4()));
        fs::create_dir_all(&root)
            .await
            .expect("failed to create root dir");

        let file_path = root.join("hello.txt");
        fs::write(&file_path, "hello world")
            .await
            .expect("failed to write file");

        let manager = FsManager::new(root.clone());
        let content = manager
            .read_text_file("hello.txt".to_string())
            .await
            .unwrap();

        assert_eq!(content, "hello world");

        fs::remove_file(&file_path)
            .await
            .expect("failed to remove file");
        fs::remove_dir_all(&root)
            .await
            .expect("failed to remove root dir");
    }

    #[tokio::test]
    async fn test_read_text_file_rejects_directory() {
        let root = env::temp_dir().join(format!("fs_root_{}", Uuid::new_v4()));
        fs::create_dir_all(&root)
            .await
            .expect("failed to create root dir");

        let dir_path = root.join("nested");
        fs::create_dir_all(&dir_path)
            .await
            .expect("failed to create nested dir");

        let manager = FsManager::new(root.clone());
        let result = manager.read_text_file("nested".to_string()).await;

        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));

        fs::remove_dir_all(&root)
            .await
            .expect("failed to remove root dir");
    }

    #[tokio::test]
    async fn test_read_text_file_rejects_large_file() {
        let root = env::temp_dir().join(format!("fs_root_{}", Uuid::new_v4()));
        fs::create_dir_all(&root)
            .await
            .expect("failed to create root dir");

        let file_path = root.join("large.txt");
        let content = vec![b'a'; (MAX_READ_BYTES as usize) + 1];
        fs::write(&file_path, content)
            .await
            .expect("failed to write file");

        let manager = FsManager::new(root.clone());
        let result = manager.read_text_file("large.txt".to_string()).await;

        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));

        fs::remove_file(&file_path)
            .await
            .expect("failed to remove file");
        fs::remove_dir_all(&root)
            .await
            .expect("failed to remove root dir");
    }

    #[tokio::test]
    async fn test_write_text_file_success() {
        let root = env::temp_dir().join(format!("fs_root_{}", Uuid::new_v4()));
        fs::create_dir_all(&root)
            .await
            .expect("failed to create root dir");

        let manager = FsManager::new(root.clone());
        let bytes = manager
            .write_text_file("hello.txt".to_string(), "hello world".to_string())
            .await
            .unwrap();

        assert_eq!(bytes, "hello world".as_bytes().len() as u64);

        let content = fs::read_to_string(root.join("hello.txt"))
            .await
            .expect("failed to read written file");
        assert_eq!(content, "hello world");

        fs::remove_file(root.join("hello.txt"))
            .await
            .expect("failed to remove file");
        fs::remove_dir_all(&root)
            .await
            .expect("failed to remove root dir");
    }

    #[tokio::test]
    async fn test_write_text_file_rejects_directory() {
        let root = env::temp_dir().join(format!("fs_root_{}", Uuid::new_v4()));
        fs::create_dir_all(&root)
            .await
            .expect("failed to create root dir");

        let dir_path = root.join("nested");
        fs::create_dir_all(&dir_path)
            .await
            .expect("failed to create nested dir");

        let manager = FsManager::new(root.clone());
        let result = manager
            .write_text_file("nested".to_string(), "data".to_string())
            .await;

        assert!(matches!(result, Err(ApiError::InvalidInput { .. })));

        fs::remove_dir_all(&root)
            .await
            .expect("failed to remove root dir");
    }
}
