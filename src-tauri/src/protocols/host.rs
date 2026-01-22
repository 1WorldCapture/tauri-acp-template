//! AgentHost trait: protocol → runtime callback boundary.
//!
//! This trait defines how protocol implementations (e.g., AcpAgent) call back
//! into the runtime layer for status updates.
//!
//! Key design principle: The protocol layer does NOT hold workspaceId/agentId.
//! These are captured by the runtime's AgentHost implementation and attached
//! to events when emitting to the frontend.

use async_trait::async_trait;

use crate::api::types::{
    AcpSessionUpdate, AgentRuntimeStatus, ApiError, OperationId, PermissionDecision,
    PermissionSource, SessionId, TerminalId,
};

/// Permission request from a protocol adapter.
#[derive(Debug, Clone)]
pub struct PermissionRequest {
    pub source: PermissionSource,
    pub session_id: Option<SessionId>,
    pub tool_call_id: Option<String>,
    pub operation_id: Option<OperationId>,
}

/// Terminal run request from a protocol adapter.
#[derive(Debug, Clone)]
pub struct TerminalRunRequest {
    pub command: String,
    pub operation_id: Option<OperationId>,
}

/// Terminal run result returned to the protocol adapter.
#[derive(Debug, Clone)]
pub struct TerminalRunResult {
    pub terminal_id: TerminalId,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// File read request from a protocol adapter.
#[derive(Debug, Clone)]
pub struct FsReadTextFileRequest {
    pub path: String,
    pub session_id: Option<SessionId>,
    pub tool_call_id: Option<String>,
    pub operation_id: Option<OperationId>,
}

/// File read result returned to the protocol adapter.
#[derive(Debug, Clone)]
pub struct FsReadTextFileResult {
    pub content: String,
}

/// File write request from a protocol adapter.
#[derive(Debug, Clone)]
pub struct FsWriteTextFileRequest {
    pub path: String,
    pub content: String,
    pub session_id: Option<SessionId>,
    pub tool_call_id: Option<String>,
    pub operation_id: Option<OperationId>,
}

/// File write result returned to the protocol adapter.
#[derive(Debug, Clone)]
pub struct FsWriteTextFileResult;

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
#[async_trait]
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

    /// Request permission from the user and await decision (US-08).
    async fn request_permission(
        &self,
        request: PermissionRequest,
    ) -> Result<PermissionDecision, ApiError>;

    /// Execute a terminal command (US-08).
    async fn terminal_run(
        &self,
        request: TerminalRunRequest,
    ) -> Result<TerminalRunResult, ApiError>;

    /// Read a text file (US-10).
    async fn fs_read_text_file(
        &self,
        request: FsReadTextFileRequest,
    ) -> Result<FsReadTextFileResult, ApiError>;

    /// Write a text file (US-11).
    async fn fs_write_text_file(
        &self,
        request: FsWriteTextFileRequest,
    ) -> Result<FsWriteTextFileResult, ApiError>;
}
