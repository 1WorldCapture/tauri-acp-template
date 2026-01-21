//! Permission Hub - manages permission request queue with async responses.
//!
//! The PermissionHub is a global singleton that handles permission requests from
//! background tasks (like plugin installation) and awaits user decisions via
//! tokio oneshot channels.
//!
//! # Flow
//!
//! 1. Background task calls `request()` with operation details
//! 2. PermissionHub emits `acp/permission_requested` event to frontend
//! 3. Background task awaits the oneshot receiver
//! 4. User responds via `permission_respond` command
//! 5. PermissionHub calls `respond()` which sends decision through oneshot
//! 6. Background task receives decision and proceeds accordingly

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex};
use tokio::time::timeout;

/// Default timeout for permission requests (5 minutes)
const PERMISSION_TIMEOUT: Duration = Duration::from_secs(300);

use crate::api::types::{
    AcpPermissionRequestedEvent, ApiError, OperationId, PermissionDecision, PermissionOrigin,
    PermissionSource,
};

/// Event name for permission requests
pub const EVENT_PERMISSION_REQUESTED: &str = "acp/permission_requested";

/// A pending permission request awaiting user decision
struct PendingPermission {
    /// Oneshot sender to deliver the decision
    tx: oneshot::Sender<PermissionDecision>,
}

/// Global permission hub for managing permission requests and responses.
///
/// This singleton is injected via `app.manage(Arc::new(PermissionHub::new(...)))`.
pub struct PermissionHub {
    /// Tauri app handle for emitting events
    app: AppHandle,
    /// Pending permission requests keyed by operation ID
    pending: Mutex<HashMap<OperationId, PendingPermission>>,
}

impl PermissionHub {
    /// Create a new PermissionHub instance.
    ///
    /// # Arguments
    ///
    /// * `app` - Tauri application handle for event emission
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// Request permission for an operation.
    ///
    /// This method:
    /// 1. Creates a oneshot channel for the response
    /// 2. Stores the sender in the pending map
    /// 3. Emits `acp/permission_requested` event to the frontend
    /// 4. Awaits and returns the user's decision
    ///
    /// # Arguments
    ///
    /// * `operation_id` - Unique identifier for this operation (UUID v4)
    /// * `source` - What is being requested (e.g., InstallPlugin)
    /// * `origin` - Optional context about where the request originated
    ///
    /// # Returns
    ///
    /// The user's decision (AllowOnce or Deny), or an error if the channel was dropped.
    pub async fn request(
        &self,
        operation_id: OperationId,
        source: PermissionSource,
        origin: Option<PermissionOrigin>,
    ) -> Result<PermissionDecision, ApiError> {
        let (tx, rx) = oneshot::channel();

        // Store the sender
        {
            let mut pending = self.pending.lock().await;
            pending.insert(operation_id.clone(), PendingPermission { tx });
        }

        // Build event payload
        let requested_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);

        let event = AcpPermissionRequestedEvent {
            operation_id: operation_id.clone(),
            source,
            requested_at_ms,
            origin,
        };

        // Emit event to frontend
        if let Err(e) = self.app.emit(EVENT_PERMISSION_REQUESTED, &event) {
            // Clean up on emit failure
            let mut pending = self.pending.lock().await;
            pending.remove(&operation_id);
            return Err(ApiError::IoError {
                message: format!("Failed to emit permission request event: {e}"),
            });
        }

        log::debug!("Permission requested: operation_id={operation_id}");

        // Await the decision with timeout to prevent indefinite hangs
        match timeout(PERMISSION_TIMEOUT, rx).await {
            Ok(Ok(decision)) => {
                log::debug!("Permission decision received: operation_id={operation_id}, decision={decision:?}");
                Ok(decision)
            }
            Ok(Err(_)) => {
                // Channel was dropped without sending - this shouldn't happen in normal flow
                log::warn!(
                    "Permission channel dropped without response: operation_id={operation_id}"
                );
                Err(ApiError::IoError {
                    message: "Permission request was cancelled".to_string(),
                })
            }
            Err(_) => {
                // Timeout - clean up pending entry and report error
                {
                    let mut pending = self.pending.lock().await;
                    pending.remove(&operation_id);
                }
                log::warn!("Permission request timed out: operation_id={operation_id}");
                Err(ApiError::IoError {
                    message: "Permission request timed out".to_string(),
                })
            }
        }
    }

    /// Respond to a pending permission request.
    ///
    /// This method looks up the pending request by operation ID and sends
    /// the user's decision through the oneshot channel.
    ///
    /// # Arguments
    ///
    /// * `operation_id` - The operation to respond to
    /// * `decision` - The user's decision (AllowOnce or Deny)
    ///
    /// # Errors
    ///
    /// - `OperationNotFound` if no pending request exists for this ID
    /// - `OperationAlreadyResolved` if the request was already responded to
    pub async fn respond(
        &self,
        operation_id: OperationId,
        decision: PermissionDecision,
    ) -> Result<(), ApiError> {
        let pending_op = {
            let mut pending = self.pending.lock().await;
            pending.remove(&operation_id)
        };

        match pending_op {
            Some(pending) => {
                // Send the decision - if this fails, the receiver was already dropped
                // (e.g., request timed out or was cancelled)
                if pending.tx.send(decision).is_err() {
                    log::warn!(
                        "Failed to send permission decision (receiver dropped): operation_id={operation_id}"
                    );
                    return Err(ApiError::OperationNotFound { operation_id });
                }
                log::info!(
                    "Permission responded: operation_id={operation_id}, decision={decision:?}"
                );
                Ok(())
            }
            None => {
                // Operation not found: either never existed, already resolved, or timed out
                log::warn!("Permission respond for unknown operation: operation_id={operation_id}");
                Err(ApiError::OperationNotFound { operation_id })
            }
        }
    }
}

// Note: Testing PermissionHub requires a real AppHandle for event emission.
// Unit testing this module would require refactoring to inject the event emitter
// behind a trait. For now, integration tests should cover the permission flow.
