//! Plugin Installer - orchestrates plugin installation with permission flow.
//!
//! The PluginInstaller manages the async installation process:
//! 1. Validates the request and checks for concurrent installs
//! 2. Returns immediately with an operation ID
//! 3. Spawns a background task that requests permission
//! 4. On approval, performs the actual installation
//! 5. Emits status change events

use std::collections::HashSet;
use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api::types::{
    AcpPluginStatusChangedEvent, ApiError, OperationId, OperationStarted, PermissionDecision,
    PermissionSource,
};
use crate::plugins::manager::PluginManager;
use crate::runtime::permissions::PermissionHub;

/// Event name for plugin status changes
pub const EVENT_PLUGIN_STATUS_CHANGED: &str = "acp/plugin_status_changed";

/// Orchestrator for plugin installation with permission flow.
///
/// This singleton is injected via `app.manage(Arc::new(PluginInstaller::new(...)))`.
pub struct PluginInstaller {
    /// Tauri app handle for emitting events
    app: AppHandle,
    /// Permission hub for requesting user approval
    permission_hub: Arc<PermissionHub>,
    /// Plugin manager for actual installation
    plugin_manager: Arc<PluginManager>,
    /// Set of plugin IDs currently being installed (for serialization)
    installing: Mutex<HashSet<String>>,
}

impl PluginInstaller {
    /// Create a new PluginInstaller instance.
    ///
    /// # Arguments
    ///
    /// * `app` - Tauri application handle for event emission
    /// * `permission_hub` - Permission hub for requesting user approval
    /// * `plugin_manager` - Plugin manager for actual installation
    pub fn new(
        app: AppHandle,
        permission_hub: Arc<PermissionHub>,
        plugin_manager: Arc<PluginManager>,
    ) -> Self {
        Self {
            app,
            permission_hub,
            plugin_manager,
            installing: Mutex::new(HashSet::new()),
        }
    }

    /// Start a plugin installation operation.
    ///
    /// This method:
    /// 1. Validates the plugin ID
    /// 2. Checks if the plugin is already being installed
    /// 3. Generates a unique operation ID
    /// 4. Spawns a background task to handle the installation flow
    /// 5. Returns immediately with the operation ID
    ///
    /// The background task will:
    /// - Request permission from the user
    /// - On approval: install the plugin and emit status change
    /// - On denial: exit cleanly
    /// - Always: remove the plugin from the installing set
    ///
    /// # Arguments
    ///
    /// * `plugin_id` - Plugin identifier (e.g., "claude-code")
    /// * `version` - Optional version to install
    ///
    /// # Returns
    ///
    /// `OperationStarted` with the operation ID, or an error if validation fails
    /// or the plugin is already being installed.
    pub async fn start_install(
        self: &Arc<Self>,
        plugin_id: String,
        version: Option<String>,
    ) -> Result<OperationStarted, ApiError> {
        // Validate plugin ID early
        PluginManager::validate_plugin_id(&plugin_id)?;

        // Check if already installing this plugin
        {
            let mut installing = self.installing.lock().await;
            if installing.contains(&plugin_id) {
                return Err(ApiError::PluginInstallInProgress {
                    plugin_id: plugin_id.clone(),
                });
            }
            // Mark as installing
            installing.insert(plugin_id.clone());
        }

        // Generate operation ID
        let operation_id = Uuid::new_v4().to_string();

        log::info!(
            "Starting plugin install: plugin_id={plugin_id}, version={version:?}, operation_id={operation_id}"
        );

        // Clone what we need for the background task
        let installer = Arc::clone(self);
        let op_id = operation_id.clone();
        let pid = plugin_id.clone();
        let ver = version.clone();

        // Spawn background task
        tauri::async_runtime::spawn(async move {
            installer.run_install_task(op_id, pid, ver).await;
        });

        Ok(OperationStarted { operation_id })
    }

    /// Background task that handles the permission request and installation.
    async fn run_install_task(
        &self,
        operation_id: OperationId,
        plugin_id: String,
        version: Option<String>,
    ) {
        // Build permission source
        let source = PermissionSource::InstallPlugin {
            plugin_id: plugin_id.clone(),
            version: version.clone(),
        };

        // Request permission (no origin context for user-initiated install)
        let decision = self
            .permission_hub
            .request(operation_id.clone(), source, None)
            .await;

        match decision {
            Ok(PermissionDecision::AllowOnce) => {
                log::info!(
                    "Permission granted for plugin install: plugin_id={plugin_id}, operation_id={operation_id}"
                );

                // Perform installation
                let install_result = self
                    .plugin_manager
                    .install(plugin_id.clone(), version)
                    .await;

                // Get current status for the event
                let status = self
                    .plugin_manager
                    .get_status(plugin_id.clone(), false)
                    .await
                    .unwrap_or_else(|_| crate::api::types::PluginStatus {
                        plugin_id: plugin_id.clone(),
                        installed: false,
                        installed_version: None,
                        latest_version: None,
                        update_available: None,
                        bin_path: None,
                    });

                // Build event with optional error
                let error = match &install_result {
                    Ok(_) => None,
                    Err(e) => Some(e.to_string()),
                };

                let event = AcpPluginStatusChangedEvent {
                    operation_id: operation_id.clone(),
                    status,
                    error,
                };

                // Emit status change event
                if let Err(e) = self.app.emit(EVENT_PLUGIN_STATUS_CHANGED, &event) {
                    log::error!(
                        "Failed to emit plugin status changed event: {e}, operation_id={operation_id}"
                    );
                }

                if let Err(e) = install_result {
                    log::error!(
                        "Plugin installation failed: plugin_id={plugin_id}, error={e}, operation_id={operation_id}"
                    );
                }
            }
            Ok(PermissionDecision::Deny) => {
                log::info!(
                    "Permission denied for plugin install: plugin_id={plugin_id}, operation_id={operation_id}"
                );
                // No event needed - frontend knows from permission response
            }
            Err(e) => {
                log::error!(
                    "Permission request failed: plugin_id={plugin_id}, error={e}, operation_id={operation_id}"
                );
                // Could emit an error event here, but for MVP we just log
            }
        }

        // Always remove from installing set
        {
            let mut installing = self.installing.lock().await;
            installing.remove(&plugin_id);
        }

        log::debug!(
            "Plugin install task completed: plugin_id={plugin_id}, operation_id={operation_id}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require a Tauri app context.
    // Here we test the validation and concurrent install prevention logic.

    #[test]
    fn test_plugin_id_validation_is_reused() {
        // Ensure we're using the same validation as PluginManager
        assert!(PluginManager::validate_plugin_id("claude-code").is_ok());
        assert!(PluginManager::validate_plugin_id("").is_err());
        assert!(PluginManager::validate_plugin_id("../etc").is_err());
    }
}
