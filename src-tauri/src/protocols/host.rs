//! AgentHost trait: protocol → runtime callback boundary.
//!
//! This trait defines how protocol implementations (e.g., AcpAgent) call back
//! into the runtime layer for status updates.
//!
//! Key design principle: The protocol layer does NOT hold workspaceId/agentId.
//! These are captured by the runtime's AgentHost implementation and attached
//! to events when emitting to the frontend.

use crate::api::types::{AcpSessionUpdate, AgentRuntimeStatus, SessionId};

/// Callback interface for protocol implementations to interact with runtime.
///
/// Implemented by the runtime layer (RuntimeAgentHost) and passed to protocol
/// implementations during connection setup. This allows AcpAgent to emit events
/// without knowing about workspace/agent business concepts.
///
/// US-06: Only `set_status()` is needed for lazy startup.
/// US-07: Adds `on_session_update()` method for streaming session updates.
///        Adds `on_connection_lost()` for process exit cleanup.
/// US-08/10/11: Will add permission and capability methods.
pub trait AgentHost: Send + Sync {
    /// Update the agent's runtime status.
    ///
    /// Called by protocol implementation when status changes (Starting, Running, Errored).
    /// The runtime implementation emits `agent/status_changed` event to frontend.
    fn set_status(&self, status: AgentRuntimeStatus);

    /// Handle a session update from the protocol.
    ///
    /// US-07: Called by protocol implementation when session updates are received
    /// (message chunks, tool calls, plans, etc.). The runtime implementation
    /// emits `acp/session_update` event to frontend with workspace/agent context.
    ///
    /// # Arguments
    /// * `session_id` - The session this update belongs to
    /// * `update` - The session update payload
    fn on_session_update(&self, session_id: SessionId, update: AcpSessionUpdate);

    /// Notify that the connection has been lost (process exited).
    ///
    /// US-07: Called when stdout/stderr close, indicating process termination.
    /// The runtime implementation should clean up connection state.
    /// This is advisory - the runtime may choose to keep state for debugging.
    fn on_connection_lost(&self);
}
