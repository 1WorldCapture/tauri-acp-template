//! Protocol layer - abstractions and implementations for agent communication.
//!
//! This module provides:
//! - `AgentConnection` trait: runtime → protocol abstraction boundary
//! - `AgentHost` trait: protocol → runtime callback boundary
//! - `acp`: ACP protocol implementation (STDIO + JSON-RPC)
//!
//! Key design principle: The protocol layer does NOT hold business concepts
//! (workspaceId, agentId). These are captured by the AgentHost implementation
//! which is provided by the runtime layer.

pub mod acp;
pub mod agent_connection;
pub mod host;

// Re-exports for external use (used by runtime layer)
#[allow(unused_imports)]
pub use agent_connection::AgentConnection;
#[allow(unused_imports)]
pub use host::AgentHost;
