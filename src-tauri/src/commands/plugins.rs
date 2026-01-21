//! Plugin-related Tauri commands.
//!
//! These commands provide the frontend API for plugin management operations.

use tauri::State;

use crate::api::types::{ApiError, PluginStatus};
use crate::plugins::manager::PluginManager;

/// Get the installation and update status of a plugin.
///
/// # Arguments
///
/// * `plugin_id` - Plugin identifier (e.g., "claude-code", "codex", "gemini")
/// * `check_updates` - Whether to check for available updates
///
/// # Returns
///
/// Returns `PluginStatus` with:
/// - `installed`: Whether the plugin is installed locally
/// - `installedVersion`: Version string if installed and metadata available
/// - `latestVersion`: Latest available version (if `check_updates=true` and implemented)
/// - `updateAvailable`: Whether an update is available (if `check_updates=true` and implemented)
/// - `binPath`: Path to the plugin binary if installed
///
/// # Errors
///
/// Returns `ApiError::InvalidInput` if the plugin ID is invalid.
/// Returns `ApiError::IoError` if there's a filesystem error.
#[tauri::command]
#[specta::specta]
pub async fn plugin_get_status(
    plugin_manager: State<'_, PluginManager>,
    plugin_id: String,
    check_updates: bool,
) -> Result<PluginStatus, ApiError> {
    log::debug!(
        "plugin_get_status called: plugin_id={}, check_updates={}",
        plugin_id,
        check_updates
    );

    let status = plugin_manager.get_status(plugin_id, check_updates).await?;

    log::debug!(
        "plugin_get_status result: installed={}, version={:?}",
        status.installed,
        status.installed_version
    );

    Ok(status)
}
