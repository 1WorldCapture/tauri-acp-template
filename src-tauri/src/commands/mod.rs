//! Tauri command handlers organized by domain.
//!
//! Each submodule contains related commands and their helper functions.
//! Import specific commands via their submodule (e.g., `commands::preferences::greet`).

pub mod agents;
pub mod chat;
pub mod notifications;
pub mod permissions;
pub mod plugins;
pub mod preferences;
pub mod quick_pane;
pub mod recovery;
pub mod workspaces;
