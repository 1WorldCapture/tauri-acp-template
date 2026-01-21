//! AcpAgent - ACP protocol implementation using STDIO + JSON-RPC.
//!
//! This module implements the AgentConnection trait for ACP-compatible adapters.
//! It handles:
//! - Spawning the adapter process with STDIO pipes
//! - ACP protocol lifecycle: initialize → new_session
//!
//! US-06: Only connection establishment is implemented.
//! US-07: Will add JSON-RPC prompt sending and response streaming.
//!
//! MVP Note: This is a simplified implementation demonstrating the architecture.
//! The full ACP protocol with JSON-RPC framing will be integrated when the
//! `agent_client_protocol` crate is available.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api::types::{ApiError, SessionId};
use crate::plugins::manager::PluginCommand;
use crate::protocols::agent_connection::AgentConnection;
use crate::protocols::host::AgentHost;

/// ACP protocol implementation using STDIO subprocess.
pub struct AcpAgent {
    /// The spawned child process (used by shutdown)
    #[allow(dead_code)]
    child: Mutex<Option<Child>>,
    /// Session ID assigned during new_session
    session_id: SessionId,
    /// Host for callbacks (status updates, used by stdout reader task)
    #[allow(dead_code)]
    host: Arc<dyn AgentHost>,
}

impl AcpAgent {
    /// Connect to an ACP adapter by spawning the process and initializing.
    ///
    /// US-06: This performs the connection lifecycle:
    /// 1. Spawn the adapter process with STDIO pipes
    /// 2. Return the connection and session ID
    ///
    /// US-07: Will add JSON-RPC initialize/new_session handshake.
    ///
    /// # Arguments
    /// * `cmd` - The plugin command specification (path, args, env)
    /// * `cwd` - Working directory for the adapter (workspace root)
    /// * `host` - Callback interface for events
    ///
    /// # Returns
    /// * `Ok((Arc<dyn AgentConnection>, SessionId))` - Connection and session
    /// * `Err(ApiError)` - Spawn or initialization failed
    pub async fn connect(
        cmd: PluginCommand,
        cwd: PathBuf,
        host: Arc<dyn AgentHost>,
    ) -> Result<(Arc<dyn AgentConnection>, SessionId), ApiError> {
        log::info!(
            "Connecting to ACP adapter: bin={:?}, cwd={:?}",
            cmd.path,
            cwd
        );

        // Spawn the adapter process
        let mut command = Command::new(&cmd.path);
        command
            .args(&cmd.args)
            .current_dir(&cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // Set environment variables
        for (key, value) in &cmd.env {
            command.env(key, value);
        }

        let mut child = command.spawn().map_err(|e| ApiError::IoError {
            message: format!("Failed to spawn adapter process: {e}"),
        })?;

        log::debug!("Adapter process spawned: pid={:?}", child.id());

        // Security/Robustness: Check if process exited immediately (crash on startup)
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process already exited
                log::error!(
                    "Adapter process exited immediately: exit_status={:?}",
                    status
                );
                return Err(ApiError::ProtocolError {
                    message: format!(
                        "Adapter process exited immediately with status: {:?}",
                        status.code()
                    ),
                });
            }
            Ok(None) => {
                // Process is still running - good
                log::debug!("Adapter process is running");
            }
            Err(e) => {
                log::warn!("Failed to check process status: {e}");
                // Continue anyway - the process might still be valid
            }
        }

        // Take ownership of stdio handles
        // Note: stdin will be used by US-07 for sending prompts
        let _stdin = child.stdin.take();

        let stdout = child.stdout.take().ok_or_else(|| ApiError::ProtocolError {
            message: "Failed to get stdout handle".to_string(),
        })?;

        let stderr = child.stderr.take().ok_or_else(|| ApiError::ProtocolError {
            message: "Failed to get stderr handle".to_string(),
        })?;

        // Generate session ID (MVP: locally generated; real ACP returns this from new_session)
        let session_id = Uuid::new_v4().to_string();

        // Spawn stderr reader task for logging
        let _stderr_task = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                log::debug!("[adapter stderr] {line}");
            }
            log::debug!("Adapter stderr closed");
        });

        // Spawn stdout reader task - US-06 only logs output
        // US-07: Will add session/update handling
        let host_for_stdout = host.clone();
        let session_id_for_stdout = session_id.clone();
        let _stdout_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                log::debug!("[adapter stdout] {line}");
            }
            // Stdout closed - process likely exited
            log::info!(
                "Adapter stdout closed, process may have exited: session={session_id_for_stdout}"
            );
            // Notify host that the agent has stopped
            host_for_stdout.set_status(crate::api::types::AgentRuntimeStatus::Stopped);
        });

        // MVP: Skip actual JSON-RPC initialize/new_session for now
        // In full implementation (US-07+):
        // 1. Send initialize request with capabilities
        // 2. Wait for initialize response
        // 3. Send new_session request with cwd
        // 4. Wait for new_session response with session_id
        log::info!("ACP connection established (MVP mode): session={session_id}");

        let agent = Arc::new(Self {
            child: Mutex::new(Some(child)),
            session_id: session_id.clone(),
            host,
        });

        Ok((agent, session_id))
    }
}

#[async_trait]
impl AgentConnection for AcpAgent {
    async fn shutdown(&self) -> Result<(), ApiError> {
        log::info!("Shutting down ACP connection: session={}", self.session_id);

        // Kill the child process
        // Take the child out of the mutex BEFORE awaiting to avoid holding lock across await
        let child_opt = {
            let mut child_guard = self.child.lock().await;
            child_guard.take()
        };

        if let Some(mut child) = child_opt {
            if let Err(e) = child.kill().await {
                log::warn!("Failed to kill adapter process: {e}");
            }
        }

        Ok(())
    }
}

impl Drop for AcpAgent {
    fn drop(&mut self) {
        log::debug!("AcpAgent dropped: session={}", self.session_id);
    }
}
