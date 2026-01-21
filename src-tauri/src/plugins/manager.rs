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
    fn validate_plugin_id(plugin_id: &str) -> Result<(), ApiError> {
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
                let metadata: Option<PluginInstallMetadata> =
                    tokio::task::spawn_blocking(move || {
                        std::fs::read_to_string(&metadata_path_clone)
                            .ok()
                            .and_then(|content| serde_json::from_str(&content).ok())
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
