//! Permission-related commands for the frontend.
//!
//! These commands allow the frontend to respond to permission requests
//! from background operations (like plugin installation).

use std::sync::Arc;

use tauri::State;

use crate::api::types::{ApiError, OperationId, PermissionDecision};
use crate::runtime::permissions::PermissionHub;

/// Respond to a pending permission request.
///
/// This command is called by the frontend when the user makes a decision
/// about a permission request (allow or deny).
///
/// # Arguments
///
/// * `operation_id` - The operation ID from the permission request event
/// * `decision` - The user's decision (AllowOnce or Deny)
///
/// # Returns
///
/// Ok(()) on success, or an error if the operation is not found or already resolved.
#[tauri::command]
#[specta::specta]
pub async fn permission_respond(
    permission_hub: State<'_, Arc<PermissionHub>>,
    operation_id: OperationId,
    decision: PermissionDecision,
) -> Result<(), ApiError> {
    log::info!("Permission response: operation_id={operation_id}, decision={decision:?}");
    permission_hub.respond(operation_id, decision).await
}
