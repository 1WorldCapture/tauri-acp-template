//! RuntimeAgentHost - Runtime layer implementation of AgentHost trait.
//!
//! This module provides the bridge between protocol implementations (like AcpAgent)
//! and the runtime layer. It captures workspace/agent context and translates
//! protocol callbacks into Tauri events.
//!
//! Key design principle: The AgentHost implementation holds workspace/agent context,
//! so the protocol layer never needs to know about these business concepts.

use std::sync::Arc;

use tauri::Emitter;

use crate::api::types::{
    AcpSessionUpdate, AcpSessionUpdateEvent, AgentId, AgentRuntimeStatus, AgentStatusChangedEvent,
    SessionId, WorkspaceId,
};
use crate::protocols::host::AgentHost;

/// Event name for agent status changes
pub const EVENT_AGENT_STATUS_CHANGED: &str = "agent/status_changed";

/// Event name for ACP session updates (US-07)
pub const EVENT_ACP_SESSION_UPDATE: &str = "acp/session_update";

/// Runtime implementation of AgentHost trait.
///
/// Holds workspace and agent context, enabling protocol implementations
/// to emit events without knowing about business concepts.
///
/// US-06: Only `set_status()` is needed for lazy startup.
/// US-07+: Will add more fields (workspace_root, etc.) as needed.
pub struct RuntimeAgentHost {
    /// Tauri application handle for emitting events
    app: tauri::AppHandle,
    /// Workspace ID for context
    workspace_id: WorkspaceId,
    /// Agent ID for context
    agent_id: AgentId,
}

impl RuntimeAgentHost {
    /// Create a new RuntimeAgentHost.
    ///
    /// # Arguments
    /// * `app` - Tauri application handle
    /// * `workspace_id` - The workspace this agent belongs to
    /// * `agent_id` - The agent this host is for
    pub fn new(app: tauri::AppHandle, workspace_id: WorkspaceId, agent_id: AgentId) -> Arc<Self> {
        Arc::new(Self {
            app,
            workspace_id,
            agent_id,
        })
    }
}

impl AgentHost for RuntimeAgentHost {
    fn set_status(&self, status: AgentRuntimeStatus) {
        let event = AgentStatusChangedEvent {
            workspace_id: self.workspace_id.clone(),
            agent_id: self.agent_id.clone(),
            status,
        };

        if let Err(e) = self.app.emit(EVENT_AGENT_STATUS_CHANGED, &event) {
            log::error!(
                "Failed to emit agent/status_changed event: {} (workspace={}, agent={})",
                e,
                self.workspace_id,
                self.agent_id
            );
        } else {
            log::debug!(
                "Emitted agent/status_changed: workspace={}, agent={}",
                self.workspace_id,
                self.agent_id
            );
        }
    }

    fn on_session_update(&self, session_id: SessionId, update: AcpSessionUpdate) {
        let event = AcpSessionUpdateEvent {
            workspace_id: self.workspace_id.clone(),
            agent_id: self.agent_id.clone(),
            session_id,
            update,
        };

        if let Err(e) = self.app.emit(EVENT_ACP_SESSION_UPDATE, &event) {
            log::error!(
                "Failed to emit acp/session_update event: {} (workspace={}, agent={})",
                e,
                self.workspace_id,
                self.agent_id
            );
        } else {
            log::trace!(
                "Emitted acp/session_update: workspace={}, agent={}",
                self.workspace_id,
                self.agent_id
            );
        }
    }

    fn on_connection_lost(&self) {
        log::warn!(
            "Agent connection lost: workspace={}, agent={}",
            self.workspace_id,
            self.agent_id
        );

        // Emit Stopped status to notify frontend
        // Note: AgentRuntime state (connection, session_id) is not automatically cleared
        // to avoid circular references. It will be detected on next operation attempt.
        self.set_status(AgentRuntimeStatus::Stopped);
    }
}
