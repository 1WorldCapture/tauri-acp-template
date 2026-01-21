//! Plugin Manager - handles plugin installation status and version checking.
//!
//! The PluginManager is a global singleton managed by Tauri's state system.
//! It provides methods to check plugin installation status and manage the plugin cache.
//!
//! # Cache Directory Structure
//!
//! ```text
//! app_cache_dir()/
//! └── plugins/
//!     ├── claude-code/
//!     │   └── install.json    # Plugin metadata
//!     ├── codex/
//!     │   └── install.json
//!     └── gemini/
//!         └── install.json
//! ```

use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::api::types::{ApiError, PluginStatus};

/// Plugin installation metadata stored in `install.json`.
///
/// This is an internal type used for persistence.
/// US-04 will write this file during plugin installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallMetadata {
    /// Installed version string
    pub installed_version: Option<String>,
    /// Path to the plugin binary/entry point
    pub bin_path: Option<String>,
}

/// Command specification for launching a plugin adapter.
///
/// Used by AgentRuntime to spawn the plugin process.
#[derive(Debug, Clone)]
pub struct PluginCommand {
    /// Path to the executable binary
    pub path: PathBuf,
    /// Command-line arguments
    pub args: Vec<String>,
    /// Environment variables to set
    pub env: Vec<(String, String)>,
}

/// Global plugin manager for checking installation status and managing plugins.
///
/// This singleton is injected via `app.manage(PluginManager::new(app.handle().clone()))`.
pub struct PluginManager {
    /// Tauri app handle for accessing app paths
    app: tauri::AppHandle,
    /// Lazily initialized plugins root directory
    plugins_root: OnceLock<PathBuf>,
}

impl PluginManager {
    /// Create a new PluginManager instance.
    ///
    /// # Arguments
    ///
    /// * `app` - Tauri application handle for path resolution
    pub fn new(app: tauri::AppHandle) -> Self {
        Self {
            app,
            plugins_root: OnceLock::new(),
        }
    }

    /// Get the plugins root directory, creating it if necessary.
    ///
    /// Returns `app_cache_dir()/plugins/`.
    fn plugins_root_dir(&self) -> Result<PathBuf, ApiError> {
        if let Some(root) = self.plugins_root.get() {
            return Ok(root.clone());
        }

        let cache_dir = self
            .app
            .path()
            .app_cache_dir()
            .map_err(|e| ApiError::IoError {
                message: format!("Failed to resolve app cache directory: {e}"),
            })?;

        let plugins_root = cache_dir.join("plugins");

        // Create the plugins directory if it doesn't exist
        std::fs::create_dir_all(&plugins_root).map_err(|e| ApiError::IoError {
            message: format!("Failed to create plugins directory: {e}"),
        })?;

        // Store and return - ignore if another thread beat us to it
        let _ = self.plugins_root.set(plugins_root.clone());
        Ok(self.plugins_root.get().cloned().unwrap_or(plugins_root))
    }

    /// Validate a plugin ID to prevent path traversal attacks.
    ///
    /// Valid plugin IDs:
    /// - Must be 1-64 characters
    /// - Can only contain lowercase letters, numbers, and hyphens
    /// - Cannot start or end with a hyphen
    ///
    /// This is public to allow other modules (e.g., PluginInstaller) to validate
    /// plugin IDs before starting operations.
    pub fn validate_plugin_id(plugin_id: &str) -> Result<(), ApiError> {
        if plugin_id.is_empty() {
            return Err(ApiError::InvalidInput {
                message: "Plugin ID cannot be empty".to_string(),
            });
        }

        if plugin_id.len() > 64 {
            return Err(ApiError::InvalidInput {
                message: "Plugin ID cannot exceed 64 characters".to_string(),
            });
        }

        // Check for path traversal attempts
        if plugin_id.contains('/') || plugin_id.contains('\\') || plugin_id.contains("..") {
            return Err(ApiError::InvalidInput {
                message: "Plugin ID contains invalid path characters".to_string(),
            });
        }

        // Only allow lowercase alphanumeric and hyphens
        let valid = plugin_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');

        if !valid {
            return Err(ApiError::InvalidInput {
                message: "Plugin ID can only contain lowercase letters, numbers, and hyphens"
                    .to_string(),
            });
        }

        // Cannot start or end with hyphen
        if plugin_id.starts_with('-') || plugin_id.ends_with('-') {
            return Err(ApiError::InvalidInput {
                message: "Plugin ID cannot start or end with a hyphen".to_string(),
            });
        }

        Ok(())
    }

    /// Get the status of a plugin.
    ///
    /// # Arguments
    ///
    /// * `plugin_id` - Plugin identifier (e.g., "claude-code", "codex", "gemini")
    /// * `check_updates` - Whether to check for available updates (MVP: not implemented)
    ///
    /// # Returns
    ///
    /// Returns `PluginStatus` with installation information.
    /// When `check_updates=false`, `latest_version` and `update_available` will be `None`.
    /// When `check_updates=true` (MVP), update fields are also `None` as update checking
    /// is not yet implemented.
    pub async fn get_status(
        &self,
        plugin_id: String,
        check_updates: bool,
    ) -> Result<PluginStatus, ApiError> {
        // Validate plugin ID to prevent path traversal
        Self::validate_plugin_id(&plugin_id)?;

        let plugins_root = self.plugins_root_dir()?;
        let plugin_dir = plugins_root.join(&plugin_id);

        // Check if plugin directory exists
        let installed = plugin_dir.exists() && plugin_dir.is_dir();

        // Try to read metadata if installed
        let (installed_version, bin_path) = if installed {
            let metadata_path = plugin_dir.join("install.json");
            if metadata_path.exists() {
                // Read metadata file - use spawn_blocking to avoid blocking async runtime
                let metadata_path_clone = metadata_path.clone();
                let plugin_id_clone = plugin_id.clone();
                let metadata: Option<PluginInstallMetadata> =
                    tokio::task::spawn_blocking(move || {
                        match std::fs::read_to_string(&metadata_path_clone) {
                            Ok(content) => match serde_json::from_str(&content) {
                                Ok(metadata) => Some(metadata),
                                Err(e) => {
                                    log::warn!(
                                        "Failed to parse install.json for plugin '{}': {}",
                                        plugin_id_clone,
                                        e
                                    );
                                    None
                                }
                            },
                            Err(e) => {
                                log::warn!(
                                    "Failed to read install.json for plugin '{}': {}",
                                    plugin_id_clone,
                                    e
                                );
                                None
                            }
                        }
                    })
                    .await
                    .ok()
                    .flatten();

                match metadata {
                    Some(m) => (m.installed_version, m.bin_path),
                    None => (None, None),
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // MVP: Update checking is not implemented
        // When implemented, this would make network requests to check for new versions
        let (latest_version, update_available) = if check_updates {
            // Future: implement actual update checking
            // For now, return None to indicate "not checked/not available"
            log::debug!("Update check requested for plugin '{plugin_id}', but not yet implemented");
            (None, None)
        } else {
            (None, None)
        };

        Ok(PluginStatus {
            plugin_id,
            installed,
            installed_version,
            latest_version,
            update_available,
            bin_path,
        })
    }

    /// Install or upgrade a plugin.
    ///
    /// MVP implementation: Creates the plugin directory and writes metadata.
    /// Future: Will execute actual npm install.
    ///
    /// # Arguments
    ///
    /// * `plugin_id` - Plugin identifier (e.g., "claude-code", "codex", "gemini")
    /// * `version` - Optional version to install (MVP: recorded in metadata only)
    ///
    /// # Returns
    ///
    /// Ok(()) on success, or an error if installation fails.
    pub async fn install(
        &self,
        plugin_id: String,
        version: Option<String>,
    ) -> Result<(), ApiError> {
        // Validate plugin ID
        Self::validate_plugin_id(&plugin_id)?;

        let plugins_root = self.plugins_root_dir()?;
        let plugin_dir = plugins_root.join(&plugin_id);

        // Create plugin directory
        let plugin_dir_clone = plugin_dir.clone();
        tokio::task::spawn_blocking(move || std::fs::create_dir_all(&plugin_dir_clone))
            .await
            .map_err(|e| ApiError::IoError {
                message: format!("Failed to spawn blocking task: {e}"),
            })?
            .map_err(|e| ApiError::IoError {
                message: format!("Failed to create plugin directory: {e}"),
            })?;

        // Build metadata
        let metadata = PluginInstallMetadata {
            installed_version: version.clone(),
            // MVP: No actual binary installed, just record the version
            // Future: This would be the path to the installed binary
            bin_path: None,
        };

        // Write install.json
        let metadata_path = plugin_dir.join("install.json");
        let metadata_json =
            serde_json::to_string_pretty(&metadata).map_err(|e| ApiError::IoError {
                message: format!("Failed to serialize plugin metadata: {e}"),
            })?;

        tokio::task::spawn_blocking(move || std::fs::write(&metadata_path, metadata_json))
            .await
            .map_err(|e| ApiError::IoError {
                message: format!("Failed to spawn blocking task: {e}"),
            })?
            .map_err(|e| ApiError::IoError {
                message: format!("Failed to write plugin metadata: {e}"),
            })?;

        log::info!("Plugin installed: plugin_id={plugin_id}, version={version:?}");

        Ok(())
    }

    /// Resolve the binary command for a plugin.
    ///
    /// Used by AgentRuntime during lazy startup to find the plugin executable.
    /// This method does NOT trigger installation - if the plugin is not installed,
    /// it returns `ApiError::PluginNotInstalled` to guide the frontend to US-04.
    ///
    /// # Arguments
    ///
    /// * `plugin_id` - Plugin identifier (e.g., "claude-code", "codex", "gemini")
    ///
    /// # Returns
    ///
    /// * `Ok(PluginCommand)` - Command to spawn the plugin process
    /// * `Err(ApiError::PluginNotInstalled)` - Plugin is not installed
    /// * `Err(ApiError::PluginMissingBinPath)` - Plugin is installed but has no binary path
    pub async fn resolve_bin(&self, plugin_id: String) -> Result<PluginCommand, ApiError> {
        // Validate plugin ID
        Self::validate_plugin_id(&plugin_id)?;

        let plugins_root = self.plugins_root_dir()?;
        let plugin_dir = plugins_root.join(&plugin_id);

        // Check if plugin directory exists
        if !plugin_dir.exists() || !plugin_dir.is_dir() {
            return Err(ApiError::PluginNotInstalled {
                plugin_id: plugin_id.clone(),
            });
        }

        // Read metadata file
        let metadata_path = plugin_dir.join("install.json");
        if !metadata_path.exists() {
            return Err(ApiError::PluginNotInstalled {
                plugin_id: plugin_id.clone(),
            });
        }

        let metadata_path_clone = metadata_path.clone();
        let plugin_id_for_error = plugin_id.clone();
        let metadata: PluginInstallMetadata = tokio::task::spawn_blocking(move || {
            let content =
                std::fs::read_to_string(&metadata_path_clone).map_err(|e| ApiError::IoError {
                    message: format!("Failed to read install.json: {e}"),
                })?;
            serde_json::from_str(&content).map_err(|e| ApiError::IoError {
                message: format!("Failed to parse install.json: {e}"),
            })
        })
        .await
        .map_err(|e| ApiError::IoError {
            message: format!("Failed to spawn blocking task: {e}"),
        })??;

        // Check if bin_path is present
        let bin_path_str = metadata
            .bin_path
            .ok_or_else(|| ApiError::PluginMissingBinPath {
                plugin_id: plugin_id_for_error.clone(),
            })?;

        if bin_path_str.is_empty() {
            return Err(ApiError::PluginMissingBinPath {
                plugin_id: plugin_id.clone(),
            });
        }

        // Security: Treat bin_path as relative to plugin_dir
        // If it's absolute, we still validate it's under plugin_dir after canonicalization
        let bin_path_raw = PathBuf::from(&bin_path_str);
        let bin_path = if bin_path_raw.is_absolute() {
            bin_path_raw
        } else {
            plugin_dir.join(&bin_path_raw)
        };

        // Verify the binary exists before canonicalization
        if !bin_path.exists() {
            log::warn!(
                "Plugin '{}' has bin_path '{}' but file does not exist",
                plugin_id,
                bin_path_str
            );
            return Err(ApiError::PluginMissingBinPath {
                plugin_id: plugin_id.clone(),
            });
        }

        // Security: Canonicalize both paths to resolve symlinks and ".." components
        let canonical_plugin_dir =
            plugin_dir
                .canonicalize()
                .map_err(|e| ApiError::PluginMissingBinPath {
                    plugin_id: format!("{}: failed to canonicalize plugin_dir: {}", plugin_id, e),
                })?;

        let canonical_bin =
            bin_path
                .canonicalize()
                .map_err(|e| ApiError::PluginMissingBinPath {
                    plugin_id: format!("{}: failed to canonicalize bin_path: {}", plugin_id, e),
                })?;

        // Security: Ensure the binary is under the plugin directory (prevent path traversal)
        if !canonical_bin.starts_with(&canonical_plugin_dir) {
            log::error!(
                "Security violation: Plugin '{}' bin_path '{}' resolves outside plugin directory",
                plugin_id,
                bin_path_str
            );
            return Err(ApiError::InvalidInput {
                message: format!(
                    "Plugin binary path must be within plugin directory: {}",
                    plugin_id
                ),
            });
        }

        // Security: Verify it's a regular file (not a directory or other special file)
        let metadata = canonical_bin.metadata().map_err(|e| ApiError::IoError {
            message: format!("Failed to get file metadata for plugin binary: {}", e),
        })?;

        if !metadata.is_file() {
            log::error!(
                "Plugin '{}' bin_path '{}' is not a regular file",
                plugin_id,
                bin_path_str
            );
            return Err(ApiError::InvalidInput {
                message: format!("Plugin binary must be a regular file: {}", plugin_id),
            });
        }

        log::debug!("Resolved plugin binary: plugin_id={plugin_id}, path={canonical_bin:?}");

        Ok(PluginCommand {
            path: canonical_bin,
            args: Vec::new(),
            env: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_plugin_id_valid() {
        assert!(PluginManager::validate_plugin_id("claude-code").is_ok());
        assert!(PluginManager::validate_plugin_id("codex").is_ok());
        assert!(PluginManager::validate_plugin_id("gemini").is_ok());
        assert!(PluginManager::validate_plugin_id("plugin-123").is_ok());
        assert!(PluginManager::validate_plugin_id("a").is_ok());
    }

    #[test]
    fn test_validate_plugin_id_empty() {
        let result = PluginManager::validate_plugin_id("");
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::InvalidInput { message } => {
                assert!(message.contains("empty"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_validate_plugin_id_too_long() {
        let long_id = "a".repeat(65);
        let result = PluginManager::validate_plugin_id(&long_id);
        assert!(result.is_err());
        match result.unwrap_err() {
            ApiError::InvalidInput { message } => {
                assert!(message.contains("64 characters"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_validate_plugin_id_path_traversal() {
        assert!(PluginManager::validate_plugin_id("../etc").is_err());
        assert!(PluginManager::validate_plugin_id("foo/bar").is_err());
        assert!(PluginManager::validate_plugin_id("foo\\bar").is_err());
        assert!(PluginManager::validate_plugin_id("..").is_err());
    }

    #[test]
    fn test_validate_plugin_id_invalid_chars() {
        assert!(PluginManager::validate_plugin_id("Plugin").is_err()); // uppercase
        assert!(PluginManager::validate_plugin_id("plugin_name").is_err()); // underscore
        assert!(PluginManager::validate_plugin_id("plugin.name").is_err()); // dot
        assert!(PluginManager::validate_plugin_id("plugin name").is_err()); // space
    }

    #[test]
    fn test_validate_plugin_id_hyphen_position() {
        assert!(PluginManager::validate_plugin_id("-plugin").is_err());
        assert!(PluginManager::validate_plugin_id("plugin-").is_err());
        assert!(PluginManager::validate_plugin_id("my-plugin").is_ok());
    }
}
