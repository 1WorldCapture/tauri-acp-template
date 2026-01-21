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
use uuid::Uuid;

use crate::api::types::{
    AcpSessionUpdate, AcpSessionUpdateEvent, AgentId, AgentRuntimeStatus, AgentStatusChangedEvent,
    ApiError, PermissionDecision, PermissionOrigin, SessionId, TerminalExitedEvent,
    TerminalOutputEvent, TerminalStream, WorkspaceId,
};
use crate::protocols::host::{AgentHost, PermissionRequest, TerminalRunRequest, TerminalRunResult};
use crate::runtime::permissions::PermissionHub;
use crate::runtime::terminal::{TerminalExit, TerminalManager, TerminalRunHandle};

/// Event name for agent status changes
pub const EVENT_AGENT_STATUS_CHANGED: &str = "agent/status_changed";

/// Event name for ACP session updates (US-07)
pub const EVENT_ACP_SESSION_UPDATE: &str = "acp/session_update";

/// Event name for terminal output (US-08)
pub const EVENT_TERMINAL_OUTPUT: &str = "terminal/output";

/// Event name for terminal exit (US-08)
pub const EVENT_TERMINAL_EXITED: &str = "terminal/exited";

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
    /// Permission hub for approval flow
    permission_hub: Arc<PermissionHub>,
    /// Terminal manager for command execution
    terminal_manager: Arc<TerminalManager>,
}

impl RuntimeAgentHost {
    /// Create a new RuntimeAgentHost.
    ///
    /// # Arguments
    /// * `app` - Tauri application handle
    /// * `workspace_id` - The workspace this agent belongs to
    /// * `agent_id` - The agent this host is for
    /// * `permission_hub` - Permission hub for approval flow
    /// * `terminal_manager` - Terminal manager scoped to the workspace
    pub fn new(
        app: tauri::AppHandle,
        workspace_id: WorkspaceId,
        agent_id: AgentId,
        permission_hub: Arc<PermissionHub>,
        terminal_manager: Arc<TerminalManager>,
    ) -> Arc<Self> {
        Arc::new(Self {
            app,
            workspace_id,
            agent_id,
            permission_hub,
            terminal_manager,
        })
    }
}

const OUTPUT_CAPTURE_LIMIT: usize = 64 * 1024;

#[async_trait::async_trait]
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

    async fn request_permission(
        &self,
        request: PermissionRequest,
    ) -> Result<PermissionDecision, ApiError> {
        let operation_id = request
            .operation_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let origin = PermissionOrigin {
            workspace_id: Some(self.workspace_id.clone()),
            agent_id: Some(self.agent_id.clone()),
            session_id: request.session_id.clone(),
            tool_call_id: request.tool_call_id.clone(),
        };

        self.permission_hub
            .request(operation_id.clone(), request.source, Some(origin))
            .await
    }

    async fn terminal_run(
        &self,
        request: TerminalRunRequest,
    ) -> Result<TerminalRunResult, ApiError> {
        let operation_id = request.operation_id.clone();

        let handle = self
            .terminal_manager
            .spawn_run(request.command.clone())
            .await?;

        let TerminalRunHandle {
            terminal_id,
            mut stdout_rx,
            mut stderr_rx,
            mut exit_rx,
        } = handle;
        let mut stdout_buffer = String::new();
        let mut stderr_buffer = String::new();
        let mut stdout_closed = false;
        let mut stderr_closed = false;
        let mut exit_received = false;
        let mut exit_code: Option<i32> = None;
        let mut user_stopped = false;

        while !(stdout_closed && stderr_closed && exit_received) {
            tokio::select! {
                stdout = stdout_rx.recv(), if !stdout_closed => {
                    match stdout {
                        Some(chunk) => {
                            let event = TerminalOutputEvent {
                                workspace_id: self.workspace_id.clone(),
                                agent_id: self.agent_id.clone(),
                                operation_id: operation_id.clone(),
                                terminal_id: terminal_id.clone(),
                                stream: TerminalStream::Stdout,
                                chunk: chunk.clone(),
                            };
                            if let Err(e) = self.app.emit(EVENT_TERMINAL_OUTPUT, &event) {
                                log::error!(
                                    "Failed to emit terminal/output: {e} (workspace={}, agent={}, terminal={terminal_id})",
                                    self.workspace_id,
                                    self.agent_id
                                );
                            }
                            append_capped(&mut stdout_buffer, &chunk, OUTPUT_CAPTURE_LIMIT);
                        }
                        None => stdout_closed = true,
                    }
                }
                stderr = stderr_rx.recv(), if !stderr_closed => {
                    match stderr {
                        Some(chunk) => {
                            let event = TerminalOutputEvent {
                                workspace_id: self.workspace_id.clone(),
                                agent_id: self.agent_id.clone(),
                                operation_id: operation_id.clone(),
                                terminal_id: terminal_id.clone(),
                                stream: TerminalStream::Stderr,
                                chunk: chunk.clone(),
                            };
                            if let Err(e) = self.app.emit(EVENT_TERMINAL_OUTPUT, &event) {
                                log::error!(
                                    "Failed to emit terminal/output: {e} (workspace={}, agent={}, terminal={terminal_id})",
                                    self.workspace_id,
                                    self.agent_id
                                );
                            }
                            append_capped(&mut stderr_buffer, &chunk, OUTPUT_CAPTURE_LIMIT);
                        }
                        None => stderr_closed = true,
                    }
                }
                exit = &mut exit_rx, if !exit_received => {
                    exit_received = true;
                    match exit {
                        Ok(TerminalExit { exit_code: code, user_stopped: stopped }) => {
                            exit_code = code;
                            user_stopped = stopped;
                        }
                        Err(_) => {
                            exit_code = None;
                            user_stopped = false;
                        }
                    }
                }
            }
        }

        let exited_event = TerminalExitedEvent {
            workspace_id: self.workspace_id.clone(),
            agent_id: self.agent_id.clone(),
            operation_id: operation_id.clone(),
            terminal_id: terminal_id.clone(),
            exit_code,
            user_stopped,
        };

        if let Err(e) = self.app.emit(EVENT_TERMINAL_EXITED, &exited_event) {
            log::error!(
                "Failed to emit terminal/exited: {e} (workspace={}, agent={}, terminal={terminal_id})",
                self.workspace_id,
                self.agent_id
            );
        }

        Ok(TerminalRunResult {
            terminal_id,
            exit_code,
            stdout: stdout_buffer,
            stderr: stderr_buffer,
        })
    }
}

fn append_capped(target: &mut String, chunk: &str, cap: usize) {
    if target.len() >= cap {
        return;
    }
    let remaining = cap.saturating_sub(target.len());
    if chunk.len() <= remaining {
        target.push_str(chunk);
    } else {
        let mut end = 0;
        for (idx, ch) in chunk.char_indices() {
            let next = idx + ch.len_utf8();
            if next > remaining {
                break;
            }
            end = next;
        }
        target.push_str(&chunk[..end]);
        target.push_str("\n...[truncated]");
    }
}
