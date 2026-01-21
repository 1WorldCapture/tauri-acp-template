//! AgentHost trait: protocol → runtime callback boundary.
//!
//! This trait defines how protocol implementations (e.g., AcpAgent) call back
//! into the runtime layer for status updates.
//!
//! Key design principle: The protocol layer does NOT hold workspaceId/agentId.
//! These are captured by the runtime's AgentHost implementation and attached
//! to events when emitting to the frontend.

use crate::api::types::AgentRuntimeStatus;

/// Callback interface for protocol implementations to interact with runtime.
///
/// Implemented by the runtime layer (RuntimeAgentHost) and passed to protocol
/// implementations during connection setup. This allows AcpAgent to emit events
/// without knowing about workspace/agent business concepts.
///
/// US-06: Only `set_status()` is needed for lazy startup.
/// US-07: Will add `on_session_update_raw()` method.
/// US-08/10/11: Will add permission and capability methods.
pub trait AgentHost: Send + Sync {
    /// Update the agent's runtime status.
    ///
    /// Called by protocol implementation when status changes (Starting, Running, Errored).
    /// The runtime implementation emits `agent/status_changed` event to frontend.
    fn set_status(&self, status: AgentRuntimeStatus);
}
