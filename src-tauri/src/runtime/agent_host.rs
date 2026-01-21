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

use crate::api::types::{AgentId, AgentRuntimeStatus, AgentStatusChangedEvent, WorkspaceId};
use crate::protocols::host::AgentHost;

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

        if let Err(e) = self.app.emit("agent/status_changed", &event) {
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
}
