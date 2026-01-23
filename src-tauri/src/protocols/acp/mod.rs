//! ACP protocol implementation.
//!
//! This module implements the Agent Client Protocol using STDIO + JSON-RPC
//! to communicate with adapter binaries (claude-code, codex, gemini, etc.).

mod agent;
mod update_mapping;

pub use agent::AcpAgent;
