//! AgentConnection trait: runtime → protocol abstraction boundary.
//!
//! This trait defines how the runtime layer interacts with agent connections
//! without knowing protocol-specific details (ACP, MCP, etc.).

use crate::api::types::{ApiError, SessionId};
use async_trait::async_trait;

/// Abstract interface for agent protocol connections.
///
/// Implemented by protocol-specific agents (e.g., AcpAgent).
/// The runtime layer uses this trait to control agent lifecycle
/// without coupling to any specific protocol implementation.
///
/// US-06: Only `shutdown()` is needed for lazy startup.
/// US-07: Adds `send_prompt()` method for sending user prompts.
/// US-12: Will add `cancel_turn()` method.
#[async_trait]
#[allow(dead_code)]
pub trait AgentConnection: Send + Sync {
    /// Send a prompt to the agent.
    ///
    /// US-07: Sends the user's prompt text to the agent process via the
    /// protocol-specific transport (e.g., JSON-RPC over stdin for ACP).
    /// Streaming responses arrive asynchronously via AgentHost callbacks.
    ///
    /// # Arguments
    /// * `session_id` - The session to send the prompt to
    /// * `prompt` - The user's prompt text
    ///
    /// # Returns
    /// * `Ok(())` - Prompt sent successfully
    /// * `Err(ApiError)` - Protocol error, connection closed, etc.
    async fn send_prompt(&self, session_id: SessionId, prompt: String) -> Result<(), ApiError>;

    /// Shutdown the agent connection gracefully.
    ///
    /// This should terminate the child process and clean up resources.
    ///
    /// # Returns
    /// * `Ok(())` - Shutdown completed
    /// * `Err(ApiError)` - Shutdown failed (resources may be leaked)
    async fn shutdown(&self) -> Result<(), ApiError>;
}
