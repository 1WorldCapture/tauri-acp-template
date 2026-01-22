//! Plugin-related Tauri commands.
//!
//! These commands provide the frontend API for plugin management operations.

use std::sync::Arc;

use tauri::State;

use crate::api::types::{ApiError, OperationStarted, PluginStatus};
use crate::plugins::manager::PluginManager;
use crate::runtime::plugin_installer::PluginInstaller;

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
    plugin_manager: State<'_, Arc<PluginManager>>,
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

/// Start a plugin installation operation.
///
/// This command initiates an async installation process:
/// 1. Validates the plugin ID
/// 2. Returns immediately with an operation ID
/// 3. Emits `acp/permission_requested` event for user approval
/// 4. On approval, installs the plugin and emits `acp/plugin_status_changed`
///
/// # Arguments
///
/// * `plugin_id` - Plugin identifier (e.g., "claude-code", "codex", "gemini")
/// * `version` - Optional version to install
///
/// # Returns
///
/// Returns `OperationStarted` with the operation ID for tracking.
///
/// # Errors
///
/// Returns `ApiError::InvalidInput` if the plugin ID is invalid.
/// Returns `ApiError::PluginInstallInProgress` if the plugin is already being installed.
#[tauri::command]
#[specta::specta]
pub async fn plugin_install(
    plugin_installer: State<'_, Arc<PluginInstaller>>,
    plugin_id: String,
    version: Option<String>,
) -> Result<OperationStarted, ApiError> {
    log::info!("plugin_install called: plugin_id={plugin_id}, version={version:?}");

    // Use .inner() to get &Arc<PluginInstaller> for the arbitrary self type receiver
    plugin_installer
        .inner()
        .start_install(plugin_id, version)
        .await
}
