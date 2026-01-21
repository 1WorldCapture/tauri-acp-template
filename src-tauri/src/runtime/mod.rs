//! Runtime modules for workspace and agent management.
//!
//! The runtime domain handles product-level concepts like
//! Workspace, Agent, Operation, and Permission.

pub mod agent_host;
pub mod agents;
pub mod path;
pub mod permissions;
pub mod plugin_installer;
pub mod terminal;
pub mod workspace;
pub mod workspace_manager;
