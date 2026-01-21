//! AcpAgent - ACP protocol implementation using STDIO + JSON-RPC.
//!
//! This module implements the AgentConnection trait for ACP-compatible adapters.
//! It handles:
//! - Spawning the adapter process with STDIO pipes
//! - ACP protocol lifecycle: initialize → new_session
//!
//! US-06: Only connection establishment is implemented.
//! US-07: Adds JSON-RPC prompt sending and session update streaming.
//!
//! MVP Note: This is a simplified implementation demonstrating the architecture.
//! The full ACP protocol with JSON-RPC framing will be integrated when the
//! `agent_client_protocol` crate is available.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api::types::{ApiError, SessionId};
use crate::plugins::manager::PluginCommand;
use crate::protocols::agent_connection::AgentConnection;
use crate::protocols::host::AgentHost;

/// JSON-RPC method name for sending prompts (US-07)
const METHOD_SEND_PROMPT: &str = "session/prompt";

/// JSON-RPC method name for session notifications (US-07)
const METHOD_SESSION_NOTIFICATION: &str = "session/notification";

/// ACP protocol implementation using STDIO subprocess.
pub struct AcpAgent {
    /// The spawned child process (used by shutdown)
    #[allow(dead_code)]
    child: Mutex<Option<Child>>,
    /// Standard input handle for sending prompts (US-07)
    stdin: Mutex<Option<tokio::process::ChildStdin>>,
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
        // US-07: Store stdin for sending prompts
        let stdin = child.stdin.take().ok_or_else(|| ApiError::ProtocolError {
            message: "Failed to get stdin handle".to_string(),
        })?;

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

        // Spawn stdout reader task - US-07: Parse JSON and handle session updates
        let host_for_stdout = host.clone();
        let session_id_for_stdout = session_id.clone();
        let _stdout_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                // Try to parse as JSON
                match serde_json::from_str::<serde_json::Value>(&line) {
                    Ok(json) => {
                        // Check if it's a notification (has "method" field)
                        if let Some(method) = json.get("method").and_then(|m| m.as_str()) {
                            if method == METHOD_SESSION_NOTIFICATION {
                                // Parse params as AcpSessionUpdate
                                if let Some(params) = json.get("params") {
                                    // Try to extract sessionId from the notification payload
                                    // Fall back to locally stored session_id if not present
                                    let notification_session_id = params
                                        .get("sessionId")
                                        .and_then(|s| s.as_str())
                                        .map(|s| s.to_string())
                                        .unwrap_or_else(|| session_id_for_stdout.clone());

                                    match serde_json::from_value::<
                                        crate::api::types::AcpSessionUpdate,
                                    >(params.clone())
                                    {
                                        Ok(update) => {
                                            host_for_stdout.on_session_update(
                                                notification_session_id.clone(),
                                                update,
                                            );
                                        }
                                        Err(e) => {
                                            // Parsing failed, wrap as Raw
                                            log::debug!(
                                                "Failed to parse session update, using Raw: {e}"
                                            );
                                            let raw_update =
                                                crate::api::types::AcpSessionUpdate::Raw {
                                                    json: params.clone(),
                                                };
                                            host_for_stdout.on_session_update(
                                                notification_session_id,
                                                raw_update,
                                            );
                                        }
                                    }
                                } else {
                                    log::debug!(
                                        "[acp] Session notification missing params: {method}"
                                    );
                                }
                            } else {
                                log::debug!("[acp] Unknown notification method: {method}");
                            }
                        } else if json.get("id").is_some() {
                            // It's a response (has "id" field), log for now
                            log::debug!("[acp stdout response] {line}");
                        } else {
                            log::debug!("[acp stdout] {line}");
                        }
                    }
                    Err(_) => {
                        // Not JSON, likely plain log output
                        log::debug!("[adapter stdout] {line}");
                    }
                }
            }

            // Stdout closed - process likely exited
            log::info!(
                "Adapter stdout closed, process may have exited: session={session_id_for_stdout}"
            );
            // Notify host that the connection has been lost and agent has stopped
            host_for_stdout.on_connection_lost();
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
            stdin: Mutex::new(Some(stdin)),
            session_id: session_id.clone(),
            host,
        });

        Ok((agent, session_id))
    }
}

#[async_trait]
impl AgentConnection for AcpAgent {
    async fn send_prompt(&self, session_id: SessionId, prompt: String) -> Result<(), ApiError> {
        log::info!(
            "Sending prompt to ACP agent: session={}, prompt_len={}",
            session_id,
            prompt.len()
        );

        // Construct JSON-RPC request (simplified, no id tracking)
        // Note: Using camelCase for params to match Rust/TS conventions and ACP spec
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": METHOD_SEND_PROMPT,
            "params": {
                "sessionId": session_id,
                "text": prompt
            }
        });

        let message = serde_json::to_string(&request).map_err(|e| ApiError::ProtocolError {
            message: format!("Failed to serialize prompt: {e}"),
        })?;

        // Write to stdin (newline-delimited JSON)
        let mut stdin_guard = self.stdin.lock().await;
        if let Some(stdin) = stdin_guard.as_mut() {
            stdin
                .write_all(message.as_bytes())
                .await
                .map_err(|e| ApiError::IoError {
                    message: format!("Failed to write to stdin: {e}"),
                })?;

            stdin
                .write_all(b"\n")
                .await
                .map_err(|e| ApiError::IoError {
                    message: format!("Failed to write newline: {e}"),
                })?;

            stdin.flush().await.map_err(|e| ApiError::IoError {
                message: format!("Failed to flush stdin: {e}"),
            })?;

            log::debug!("Prompt sent successfully: session={session_id}");
            Ok(())
        } else {
            Err(ApiError::ProtocolError {
                message: "stdin not available".to_string(),
            })
        }
    }

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
