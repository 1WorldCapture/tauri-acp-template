//! AgentConnection trait: runtime → protocol abstraction boundary.
//!
//! This trait defines how the runtime layer interacts with agent connections
//! without knowing protocol-specific details (ACP, MCP, etc.).

use crate::api::types::ApiError;
use async_trait::async_trait;

/// Abstract interface for agent protocol connections.
///
/// Implemented by protocol-specific agents (e.g., AcpAgent).
/// The runtime layer uses this trait to control agent lifecycle
/// without coupling to any specific protocol implementation.
///
/// US-06: Only `shutdown()` is needed for lazy startup.
/// US-07: Will add `send_prompt()` method.
/// US-12: Will add `cancel_turn()` method.
#[async_trait]
#[allow(dead_code)]
pub trait AgentConnection: Send + Sync {
    /// Shutdown the agent connection gracefully.
    ///
    /// This should terminate the child process and clean up resources.
    ///
    /// # Returns
    /// * `Ok(())` - Shutdown completed
    /// * `Err(ApiError)` - Shutdown failed (resources may be leaked)
    async fn shutdown(&self) -> Result<(), ApiError>;
}
