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

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::process::Command;

use crate::api::types::{ApiError, PluginStatus};

/// Plugin descriptor containing npm package information.
///
/// Maps plugin IDs to their npm package names and binary names.
#[derive(Debug, Clone)]
pub struct PluginDescriptor {
    /// Plugin ID (e.g., "claude-code")
    pub plugin_id: &'static str,
    /// npm package name (e.g., "@zed-industries/claude-code-acp")
    pub npm_package: &'static str,
    /// Binary name in node_modules/.bin (e.g., "claude-code-acp")
    pub bin_name: &'static str,
}

/// Known plugin descriptors registry.
const PLUGIN_REGISTRY: &[PluginDescriptor] = &[
    PluginDescriptor {
        plugin_id: "claude-code",
        npm_package: "@zed-industries/claude-code-acp",
        bin_name: "claude-code-acp",
    },
    PluginDescriptor {
        plugin_id: "codex",
        npm_package: "@zed-industries/codex-acp",
        bin_name: "codex-acp",
    },
];

/// Look up a plugin descriptor by ID.
///
/// Returns an error for unknown plugin IDs.
fn plugin_descriptor(plugin_id: &str) -> Result<&'static PluginDescriptor, ApiError> {
    PLUGIN_REGISTRY
        .iter()
        .find(|d| d.plugin_id == plugin_id)
        .ok_or_else(|| ApiError::InvalidInput {
            message: format!(
                "Unknown plugin id: '{}'. Supported plugins: {}",
                plugin_id,
                PLUGIN_REGISTRY
                    .iter()
                    .map(|d| d.plugin_id)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        })
}

/// Plugin installation metadata stored in `install.json`.
///
/// This is an internal type used for persistence.
/// Stores npm package information and resolved binary path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallMetadata {
    /// Installed version string (from package.json after npm install)
    pub installed_version: Option<String>,
    /// Relative path to the plugin binary under plugin_dir (e.g., "node_modules/.bin/claude-code-acp")
    pub bin_path: Option<String>,
    /// npm package name (e.g., "@zed-industries/claude-code-acp")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npm_package: Option<String>,
    /// Binary name (e.g., "claude-code-acp")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin_name: Option<String>,
    /// Installation timestamp in milliseconds since Unix epoch
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_at_ms: Option<f64>,
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

    // =========================================================================
    // Helper functions for npm-based plugin installation
    // =========================================================================

    /// Write a minimal package.json for npm install.
    ///
    /// Creates a package.json with a single dependency on the plugin's npm package.
    fn write_plugin_package_json(
        plugin_dir: &Path,
        plugin_id: &str,
        npm_package: &str,
        version: Option<&str>,
    ) -> Result<(), ApiError> {
        let version_spec = version.unwrap_or("latest");
        let package_json = serde_json::json!({
            "name": format!("acp-plugin-{plugin_id}"),
            "version": "1.0.0",
            "private": true,
            "dependencies": {
                npm_package: version_spec
            }
        });

        let package_json_path = plugin_dir.join("package.json");
        let content =
            serde_json::to_string_pretty(&package_json).map_err(|e| ApiError::IoError {
                message: format!("Failed to serialize package.json: {e}"),
            })?;

        std::fs::write(&package_json_path, content).map_err(|e| ApiError::IoError {
            message: format!("Failed to write package.json: {e}"),
        })?;

        log::debug!("Wrote package.json to {package_json_path:?}");
        Ok(())
    }

    /// Execute npm install in the plugin directory.
    ///
    /// Uses `npm.cmd` on Windows, `npm` on other platforms.
    async fn run_npm_install(plugin_dir: &Path) -> Result<(), ApiError> {
        // Determine npm executable based on platform
        #[cfg(windows)]
        let npm_cmd = "npm.cmd";
        #[cfg(not(windows))]
        let npm_cmd = "npm";

        log::info!("Running npm install in {plugin_dir:?}");

        let output = Command::new(npm_cmd)
            .args([
                "install",
                "--no-audit",
                "--no-fund",
                "--loglevel=error",
                "--omit=dev",
            ])
            .current_dir(plugin_dir)
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ApiError::InvalidInput {
                        message: format!(
                            "npm is not installed or not in PATH. Please install Node.js and npm first. Error: {e}"
                        ),
                    }
                } else {
                    ApiError::IoError {
                        message: format!("Failed to execute npm install: {e}"),
                    }
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let exit_code = output.status.code().unwrap_or(-1);

            // Truncate output for error message
            let max_len = 500;
            let stderr_truncated = if stderr.len() > max_len {
                format!("{}...(truncated)", &stderr[..max_len])
            } else {
                stderr.to_string()
            };

            log::error!(
                "npm install failed with exit code {exit_code}:\nstderr: {stderr}\nstdout: {stdout}"
            );

            return Err(ApiError::IoError {
                message: format!("npm install failed (exit code {exit_code}): {stderr_truncated}"),
            });
        }

        log::info!("npm install completed successfully");
        Ok(())
    }

    /// Read the installed package version from node_modules.
    ///
    /// Parses the package.json in node_modules to get the actual installed version.
    fn read_installed_npm_version(
        plugin_dir: &Path,
        npm_package: &str,
    ) -> Result<String, ApiError> {
        // For scoped packages like @zed-industries/claude-code-acp,
        // the path is node_modules/@zed-industries/claude-code-acp/package.json
        let package_path = plugin_dir
            .join("node_modules")
            .join(npm_package)
            .join("package.json");

        let content = std::fs::read_to_string(&package_path).map_err(|e| ApiError::IoError {
            message: format!("Failed to read installed package version from {package_path:?}: {e}"),
        })?;

        let package_json: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| ApiError::IoError {
                message: format!("Failed to parse installed package.json: {e}"),
            })?;

        package_json
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ApiError::IoError {
                message: "Installed package.json missing 'version' field".to_string(),
            })
    }

    /// Resolve the npm bin shim path (cross-platform).
    ///
    /// Returns a relative path under plugin_dir (e.g., "node_modules/.bin/claude-code-acp").
    fn resolve_npm_bin_relative_path(
        plugin_dir: &Path,
        bin_name: &str,
    ) -> Result<String, ApiError> {
        let bin_dir = plugin_dir.join("node_modules").join(".bin");

        // On Windows, prefer .cmd, then .exe, then plain
        #[cfg(windows)]
        let candidates = [
            format!("{bin_name}.cmd"),
            format!("{bin_name}.exe"),
            bin_name.to_string(),
        ];

        // On Unix, just use the plain name
        #[cfg(not(windows))]
        let candidates = [bin_name.to_string()];

        for candidate in &candidates {
            let full_path = bin_dir.join(candidate);
            if full_path.exists() && full_path.is_file() {
                // Return relative path with forward slashes for portability
                let relative = format!("node_modules/.bin/{candidate}");
                log::debug!("Resolved bin path: {relative}");
                return Ok(relative);
            }
        }

        Err(ApiError::PluginMissingBinPath {
            plugin_id: format!(
                "Binary '{}' not found in {:?}. Checked: {:?}",
                bin_name, bin_dir, candidates
            ),
        })
    }

    /// Write install metadata atomically.
    ///
    /// Writes to a temp file first, then renames to install.json.
    fn write_install_metadata(
        plugin_dir: &Path,
        metadata: &PluginInstallMetadata,
    ) -> Result<(), ApiError> {
        let metadata_path = plugin_dir.join("install.json");
        let temp_path = plugin_dir.join("install.json.tmp");

        let content = serde_json::to_string_pretty(metadata).map_err(|e| ApiError::IoError {
            message: format!("Failed to serialize install metadata: {e}"),
        })?;

        // Write to temp file
        std::fs::write(&temp_path, &content).map_err(|e| ApiError::IoError {
            message: format!("Failed to write temp metadata file: {e}"),
        })?;

        // Atomic rename
        std::fs::rename(&temp_path, &metadata_path).map_err(|e| ApiError::IoError {
            message: format!("Failed to rename temp metadata file: {e}"),
        })?;

        log::debug!("Wrote install.json to {metadata_path:?}");
        Ok(())
    }

    // =========================================================================
    // Main installation method
    // =========================================================================

    /// Install or upgrade a plugin via npm.
    ///
    /// This method:
    /// 1. Validates the plugin ID and looks up the npm package
    /// 2. Creates the plugin directory
    /// 3. Writes a package.json with the dependency
    /// 4. Runs `npm install`
    /// 5. Resolves the installed version and binary path
    /// 6. Writes install.json with the metadata
    ///
    /// # Arguments
    ///
    /// * `plugin_id` - Plugin identifier (e.g., "claude-code")
    /// * `version` - Optional version to install (defaults to "latest")
    ///
    /// # Returns
    ///
    /// Ok(()) on success, or an error if installation fails.
    pub async fn install(
        &self,
        plugin_id: String,
        version: Option<String>,
    ) -> Result<(), ApiError> {
        // 1. Validate plugin ID
        Self::validate_plugin_id(&plugin_id)?;

        // 2. Look up plugin descriptor (validates plugin is known)
        let desc = plugin_descriptor(&plugin_id)?;
        log::info!(
            "Installing plugin: plugin_id={plugin_id}, npm_package={}, version={:?}",
            desc.npm_package,
            version
        );

        // 3. Get plugin directory
        let plugins_root = self.plugins_root_dir()?;
        let plugin_dir = plugins_root.join(&plugin_id);

        // 4. Create plugin directory (blocking operation)
        let plugin_dir_clone = plugin_dir.clone();
        tokio::task::spawn_blocking(move || std::fs::create_dir_all(&plugin_dir_clone))
            .await
            .map_err(|e| ApiError::IoError {
                message: format!("Failed to spawn blocking task: {e}"),
            })?
            .map_err(|e| ApiError::IoError {
                message: format!("Failed to create plugin directory: {e}"),
            })?;

        // 5. Write package.json (blocking operation)
        let plugin_dir_for_package = plugin_dir.clone();
        let version_for_package = version.clone();
        let desc_npm_package = desc.npm_package.to_string();
        let desc_plugin_id = desc.plugin_id.to_string();

        tokio::task::spawn_blocking(move || {
            Self::write_plugin_package_json(
                &plugin_dir_for_package,
                &desc_plugin_id,
                &desc_npm_package,
                version_for_package.as_deref(),
            )
        })
        .await
        .map_err(|e| ApiError::IoError {
            message: format!("Failed to spawn blocking task: {e}"),
        })??;

        // 6. Run npm install (async operation)
        Self::run_npm_install(&plugin_dir).await?;

        // 7. Read installed version and resolve bin path (blocking operations)
        let plugin_dir_for_version = plugin_dir.clone();
        let npm_package_for_version = desc.npm_package.to_string();
        let installed_version = tokio::task::spawn_blocking(move || {
            Self::read_installed_npm_version(&plugin_dir_for_version, &npm_package_for_version)
        })
        .await
        .map_err(|e| ApiError::IoError {
            message: format!("Failed to spawn blocking task: {e}"),
        })??;

        let plugin_dir_for_bin = plugin_dir.clone();
        let bin_name_for_resolve = desc.bin_name.to_string();
        let bin_path = tokio::task::spawn_blocking(move || {
            Self::resolve_npm_bin_relative_path(&plugin_dir_for_bin, &bin_name_for_resolve)
        })
        .await
        .map_err(|e| ApiError::IoError {
            message: format!("Failed to spawn blocking task: {e}"),
        })??;

        // 8. Get current timestamp
        let installed_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .ok();

        // 9. Build and write metadata
        let metadata = PluginInstallMetadata {
            installed_version: Some(installed_version.clone()),
            bin_path: Some(bin_path.clone()),
            npm_package: Some(desc.npm_package.to_string()),
            bin_name: Some(desc.bin_name.to_string()),
            installed_at_ms,
        };

        let plugin_dir_for_metadata = plugin_dir.clone();
        tokio::task::spawn_blocking(move || {
            Self::write_install_metadata(&plugin_dir_for_metadata, &metadata)
        })
        .await
        .map_err(|e| ApiError::IoError {
            message: format!("Failed to spawn blocking task: {e}"),
        })??;

        log::info!(
            "Plugin installed successfully: plugin_id={plugin_id}, version={installed_version}, bin_path={bin_path}"
        );

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
