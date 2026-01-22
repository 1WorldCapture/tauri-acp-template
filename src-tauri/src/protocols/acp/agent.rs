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
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

use crate::api::types::{ApiError, PermissionSource, SessionId};
use crate::plugins::manager::PluginCommand;
use crate::protocols::agent_connection::AgentConnection;
use crate::protocols::host::{
    AgentHost, FsReadTextFileRequest, FsWriteTextFileRequest, PermissionRequest, TerminalRunRequest,
};

/// JSON-RPC method name for sending prompts (US-07)
const METHOD_SEND_PROMPT: &str = "session/prompt";

/// JSON-RPC method name for session notifications (US-07)
const METHOD_SESSION_NOTIFICATION: &str = "session/notification";

/// JSON-RPC method name for permission requests (US-08)
const METHOD_REQUEST_PERMISSION: &str = "request_permission";

/// JSON-RPC method name for terminal run requests (US-08)
const METHOD_TERMINAL_RUN: &str = "terminal/run";
/// JSON-RPC method name for file read requests (US-10)
const METHOD_FS_READ_TEXT_FILE: &str = "fs.read_text_file";
const METHOD_FS_READ_TEXT_FILE_ALIAS: &str = "read_text_file";
/// JSON-RPC method name for file write requests (US-11)
const METHOD_FS_WRITE_TEXT_FILE: &str = "fs.write_text_file";
const METHOD_FS_WRITE_TEXT_FILE_ALIAS: &str = "write_text_file";

const MAX_INFLIGHT_REQUESTS: usize = 8;

/// ACP protocol implementation using STDIO subprocess.
pub struct AcpAgent {
    /// The spawned child process (used by shutdown)
    #[allow(dead_code)]
    child: Mutex<Option<Child>>,
    /// Standard input handle for sending prompts (US-07)
    stdin: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
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

        let stdin = Arc::new(Mutex::new(Some(stdin)));

        let request_semaphore = Arc::new(Semaphore::new(MAX_INFLIGHT_REQUESTS));

        // Spawn stdout reader task - US-07: Parse JSON and handle session updates
        let host_for_stdout = host.clone();
        let session_id_for_stdout = session_id.clone();
        let stdin_for_stdout = stdin.clone();
        let semaphore_for_stdout = request_semaphore.clone();
        let _stdout_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                // Try to parse as JSON
                match serde_json::from_str::<serde_json::Value>(&line) {
                    Ok(json) => {
                        let method = json
                            .get("method")
                            .and_then(|m| m.as_str())
                            .map(|value| value.to_string());
                        let id = json.get("id").cloned();

                        if let Some(method) = method {
                            if id.is_none() {
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
                                        >(
                                            params.clone()
                                        ) {
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
                            } else {
                                let host_for_request = host_for_stdout.clone();
                                let stdin_for_request = stdin_for_stdout.clone();
                                let request_id = id.unwrap_or(serde_json::Value::Null);
                                let params = json.get("params").cloned();
                                let fallback_session_id = session_id_for_stdout.clone();
                                let semaphore_for_request = semaphore_for_stdout.clone();

                                let permit = match semaphore_for_request.acquire_owned().await {
                                    Ok(permit) => permit,
                                    Err(_) => break,
                                };

                                tokio::spawn(async move {
                                    let _permit = permit;
                                    handle_request(
                                        host_for_request,
                                        stdin_for_request,
                                        method,
                                        request_id,
                                        params,
                                        fallback_session_id,
                                    )
                                    .await;
                                });
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
            stdin,
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

async fn handle_request(
    host: Arc<dyn AgentHost>,
    stdin: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    method: String,
    id: serde_json::Value,
    params: Option<serde_json::Value>,
    fallback_session_id: SessionId,
) {
    let params = params.unwrap_or(serde_json::Value::Null);

    let response = match method.as_str() {
        METHOD_REQUEST_PERMISSION => {
            let session_id = extract_string(&params, &["sessionId", "session_id"])
                .or(Some(fallback_session_id.clone()));
            let tool_call_id = extract_string(&params, &["toolCallId", "tool_call_id"]);
            let operation_id = extract_string(&params, &["operationId", "operation_id"]);
            let command = extract_command(&params)
                .or_else(|| extract_string(&params, &["summary"]))
                .unwrap_or_else(|| "terminal command".to_string());

            let request = PermissionRequest {
                source: PermissionSource::TerminalRun { command },
                session_id,
                tool_call_id,
                operation_id,
            };

            match host.request_permission(request).await {
                Ok(decision) => {
                    let decision_value = serde_json::Value::String(match decision {
                        crate::api::types::PermissionDecision::AllowOnce => "AllowOnce".to_string(),
                        crate::api::types::PermissionDecision::Deny => "Deny".to_string(),
                    });
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": decision_value
                    })
                }
                Err(e) => jsonrpc_error(id, -32000, &e.to_string()),
            }
        }
        METHOD_TERMINAL_RUN => {
            let command = extract_command(&params);
            if command.is_none() {
                jsonrpc_error(id, -32602, "Missing command")
            } else {
                let operation_id = extract_string(&params, &["operationId", "operation_id"]);

                let request = TerminalRunRequest {
                    command: command.unwrap_or_default(),
                    operation_id,
                };

                match host.terminal_run(request).await {
                    Ok(result) => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "terminalId": result.terminal_id,
                            "exitCode": result.exit_code,
                            "stdout": result.stdout,
                            "stderr": result.stderr
                        }
                    }),
                    Err(e) => jsonrpc_error(id, -32000, &e.to_string()),
                }
            }
        }
        METHOD_FS_READ_TEXT_FILE | METHOD_FS_READ_TEXT_FILE_ALIAS => {
            let path = extract_path(&params);
            if path.is_none() {
                jsonrpc_error(id, -32602, "Missing path")
            } else {
                let session_id = extract_string(&params, &["sessionId", "session_id"])
                    .or(Some(fallback_session_id.clone()));
                let tool_call_id = extract_string(&params, &["toolCallId", "tool_call_id"]);
                let operation_id = extract_string(&params, &["operationId", "operation_id"]);

                let request = FsReadTextFileRequest {
                    path: path.unwrap_or_default(),
                    session_id,
                    tool_call_id,
                    operation_id,
                };

                match host.fs_read_text_file(request).await {
                    Ok(result) => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": result.content
                        }
                    }),
                    Err(e) => jsonrpc_error(id, -32000, &e.to_string()),
                }
            }
        }
        METHOD_FS_WRITE_TEXT_FILE | METHOD_FS_WRITE_TEXT_FILE_ALIAS => {
            let path = extract_path(&params);
            let content = extract_content(&params);
            if path.is_none() {
                jsonrpc_error(id, -32602, "Missing path")
            } else if content.is_none() {
                jsonrpc_error(id, -32602, "Missing content")
            } else {
                let session_id = extract_string(&params, &["sessionId", "session_id"])
                    .or(Some(fallback_session_id.clone()));
                let tool_call_id = extract_string(&params, &["toolCallId", "tool_call_id"]);
                let operation_id = extract_string(&params, &["operationId", "operation_id"]);

                let request = FsWriteTextFileRequest {
                    path: path.unwrap_or_default(),
                    content: content.unwrap_or_default(),
                    session_id,
                    tool_call_id,
                    operation_id,
                };

                match host.fs_write_text_file(request).await {
                    Ok(_) => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {}
                    }),
                    Err(e) => jsonrpc_error(id, -32000, &e.to_string()),
                }
            }
        }
        _ => jsonrpc_error(id, -32601, "Method not found"),
    };

    if let Err(e) = send_jsonrpc_response(&stdin, response).await {
        log::warn!("Failed to send JSON-RPC response: {e}");
    }
}

fn extract_command(params: &serde_json::Value) -> Option<String> {
    extract_string(params, &["command", "commandString", "cmd"]).or_else(|| {
        params
            .get("details")
            .and_then(|details| extract_string(details, &["command", "commandString", "cmd"]))
    })
}

fn extract_path(params: &serde_json::Value) -> Option<String> {
    extract_string(params, &["path", "filePath", "file_path"]).or_else(|| {
        params
            .get("details")
            .and_then(|details| extract_string(details, &["path", "filePath", "file_path"]))
    })
}

fn extract_content(params: &serde_json::Value) -> Option<String> {
    extract_string(params, &["content", "text"]).or_else(|| {
        params
            .get("details")
            .and_then(|details| extract_string(details, &["content", "text"]))
    })
}

fn extract_string(params: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        params
            .get(*key)
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
    })
}

fn jsonrpc_error(id: serde_json::Value, code: i64, message: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

async fn send_jsonrpc_response(
    stdin: &Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    response: serde_json::Value,
) -> Result<(), ApiError> {
    let message = serde_json::to_string(&response).map_err(|e| ApiError::ProtocolError {
        message: format!("Failed to serialize JSON-RPC response: {e}"),
    })?;

    let mut stdin_guard = stdin.lock().await;
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
        Ok(())
    } else {
        Err(ApiError::ProtocolError {
            message: "stdin not available".to_string(),
        })
    }
}
