//! AcpAgent - ACP protocol implementation using STDIO + JSON-RPC.
//!
//! This module implements the AgentConnection trait for ACP-compatible adapters.
//! It handles:
//! - Spawning the adapter process with STDIO pipes
//! - ACP protocol lifecycle: initialize → session/new → session/prompt
//! - Streaming session updates via notifications
//!
//! US-06: Connection establishment with ACP handshake
//! US-07: JSON-RPC prompt sending and session update streaming
//! US-08: Permission requests from adapter
//! US-10/11: File system read/write operations
//! US-12: Turn cancellation

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

use agent_client_protocol as acp;
use crate::api::types::{ApiError, PermissionSource, SessionId};
use crate::plugins::manager::PluginCommand;
use crate::protocols::agent_connection::AgentConnection;
use crate::protocols::host::{
    AgentHost, FsReadTextFileRequest, FsWriteTextFileRequest, PermissionRequest, TerminalRunRequest,
};
use super::update_mapping::{
    map_acp_update_to_api_update, parse_acp_session_notification_params,
};

/// JSON-RPC method name for initialize handshake
/// Per ACP protocol schema: agent-client-protocol-schema/src/agent.rs
const METHOD_INITIALIZE: &str = "initialize";
/// JSON-RPC method name for session creation
/// Per ACP protocol schema: agent-client-protocol-schema/src/agent.rs
const METHOD_SESSION_NEW: &str = "session/new";
/// JSON-RPC method name for sending prompts (US-07)
/// Per ACP protocol schema: agent-client-protocol-schema/src/agent.rs
const METHOD_SEND_PROMPT: &str = "session/prompt";
/// JSON-RPC method name for canceling the current turn (US-12)
/// Per ACP protocol schema: agent-client-protocol-schema/src/agent.rs
const METHOD_CANCEL_TURN: &str = "session/cancel";

/// JSON-RPC method name for session notifications (US-07)
const METHOD_SESSION_NOTIFICATION: &str = "session/notification";
/// Alias for session notifications (some adapters use this)
const METHOD_SESSION_UPDATE: &str = "session/update";

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

        // Perform ACP handshake: initialize → session/new
        // This must happen before spawning the stdout reader task
        let mut stdout_reader = BufReader::new(stdout);
        let session_id =
            perform_acp_handshake(&stdin, &mut stdout_reader, &cwd).await?;

        log::info!("ACP handshake completed: session={session_id}");

        // Now spawn stdout reader task for ongoing notifications/requests
        let request_semaphore = Arc::new(Semaphore::new(MAX_INFLIGHT_REQUESTS));
        let host_for_stdout = host.clone();
        let session_id_for_stdout = session_id.clone();
        let stdin_for_stdout = stdin.clone();
        let semaphore_for_stdout = request_semaphore.clone();

        // Convert stdout_reader back to lines iterator for the task
        let _stdout_task = tokio::spawn(async move {
            let mut lines = stdout_reader.lines();

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
                                // Handle session notifications (both method names for compatibility)
                                if method == METHOD_SESSION_NOTIFICATION
                                    || method == METHOD_SESSION_UPDATE
                                {
                                    if let Some(params) = json.get("params").cloned() {
                                        match parse_acp_session_notification_params(
                                            params.clone(),
                                            &session_id_for_stdout,
                                        ) {
                                            Ok((notification_session_id, update)) => {
                                                let api_update =
                                                    map_acp_update_to_api_update(update);
                                                host_for_stdout.on_session_update(
                                                    notification_session_id,
                                                    api_update,
                                                );
                                            }
                                            Err(e) => {
                                                let raw_session_id = extract_string(
                                                    &params,
                                                    &["sessionId", "session_id"],
                                                )
                                                .unwrap_or_else(|| session_id_for_stdout.clone());
                                                log::debug!(
                                                    "Failed to parse ACP session update, using Raw: {e}"
                                                );
                                                let raw_update =
                                                    crate::api::types::AcpSessionUpdate::Raw {
                                                        json: params,
                                                    };
                                                host_for_stdout.on_session_update(
                                                    raw_session_id,
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
                            if let Some(result) = json.get("result") {
                                let stop_reason_value = result
                                    .get("stopReason")
                                    .or_else(|| result.get("stop_reason"))
                                    .cloned();
                                if let Some(stop_reason_value) = stop_reason_value {
                                    let stop_reason = match serde_json::from_value::<
                                        acp::PromptResponse,
                                    >(result.clone())
                                    {
                                        Ok(prompt_response) => serde_json::to_value(
                                            prompt_response.stop_reason,
                                        )
                                        .unwrap_or(stop_reason_value.clone()),
                                        Err(e) => {
                                            log::debug!(
                                                "Failed to parse prompt response stopReason: {e}"
                                            );
                                            stop_reason_value
                                        }
                                    };
                                    let update =
                                        crate::api::types::AcpSessionUpdate::TurnComplete {
                                            stop_reason,
                                        };
                                    host_for_stdout.on_session_update(
                                        session_id_for_stdout.clone(),
                                        update,
                                    );
                                }
                            }
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

        log::info!("ACP connection established: session={session_id}");

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

        // Generate a unique request ID for JSON-RPC
        let request_id = Uuid::new_v4().to_string();

        // Construct JSON-RPC request with proper ACP PromptRequest structure
        // ACP expects: { sessionId: string, prompt: ContentBlock[] }
        // where ContentBlock can be { type: "text", text: string } etc.
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": METHOD_SEND_PROMPT,
            "params": {
                "sessionId": session_id,
                "prompt": [{
                    "type": "text",
                    "text": prompt
                }]
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

    async fn cancel_turn(&self, session_id: SessionId) -> Result<(), ApiError> {
        log::info!("Canceling turn for ACP session: {}", session_id);

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": METHOD_CANCEL_TURN,
            "params": {
                "sessionId": session_id
            }
        });

        let message = serde_json::to_string(&request).map_err(|e| ApiError::ProtocolError {
            message: format!("Failed to serialize cancel request: {e}"),
        })?;

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

            log::debug!("Cancel request sent successfully: session={session_id}");
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

/// Perform ACP handshake: initialize → session/new
///
/// This sends the required ACP protocol messages to establish a session:
/// 1. `initialize` - Handshake with protocol version and capabilities
/// 2. `session/new` - Create a new session with workspace cwd
///
/// Returns the adapter-issued session ID.
async fn perform_acp_handshake(
    stdin: &Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    stdout: &mut BufReader<tokio::process::ChildStdout>,
    cwd: &PathBuf,
) -> Result<SessionId, ApiError> {
    // Step 1: Send initialize request
    let init_id = Uuid::new_v4().to_string();
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": init_id,
        "method": METHOD_INITIALIZE,
        "params": {
            "protocolVersion": 1,
            "clientCapabilities": {
                "sampling": {},
                "roots": { "listChanged": false },
                "prompts": {},
                "tools": {},
                "logging": {}
            },
            "clientInfo": {
                "name": "tauri-acp-template",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    });

    write_jsonrpc_request(stdin, &init_request).await?;
    log::debug!("Sent initialize request: id={init_id}");

    // Wait for initialize response
    let init_response = read_jsonrpc_response(stdout, &init_id).await?;
    log::debug!("Received initialize response: {init_response}");

    // Check for error in response
    if let Some(error) = init_response.get("error") {
        let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(ApiError::ProtocolError {
            message: format!("Initialize failed: code={code}, message={message}"),
        });
    }

    // Step 2: Send session/new request
    let session_id_request = Uuid::new_v4().to_string();
    let cwd_str = cwd.to_string_lossy().to_string();
    let session_new_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": session_id_request,
        "method": METHOD_SESSION_NEW,
        "params": {
            "cwd": cwd_str,
            "mcpServers": []
        }
    });

    write_jsonrpc_request(stdin, &session_new_request).await?;
    log::debug!("Sent session/new request: id={session_id_request}, cwd={cwd_str}");

    // Wait for session/new response
    let session_response = read_jsonrpc_response(stdout, &session_id_request).await?;
    log::debug!("Received session/new response: {session_response}");

    // Check for error in response
    if let Some(error) = session_response.get("error") {
        let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(ApiError::ProtocolError {
            message: format!("Session creation failed: code={code}, message={message}"),
        });
    }

    // Extract session ID from response
    let session_id = session_response
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| ApiError::ProtocolError {
            message: "session/new response missing sessionId".to_string(),
        })?;

    Ok(session_id)
}

/// Write a JSON-RPC request to stdin (newline-delimited JSON)
async fn write_jsonrpc_request(
    stdin: &Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    request: &serde_json::Value,
) -> Result<(), ApiError> {
    let message = serde_json::to_string(request).map_err(|e| ApiError::ProtocolError {
        message: format!("Failed to serialize JSON-RPC request: {e}"),
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

/// Read JSON-RPC responses from stdout until we find one with the expected ID.
/// Non-matching lines (notifications, other responses) are logged and skipped.
async fn read_jsonrpc_response(
    stdout: &mut BufReader<tokio::process::ChildStdout>,
    expected_id: &str,
) -> Result<serde_json::Value, ApiError> {
    let mut lines = stdout.lines();

    loop {
        let line = lines.next_line().await.map_err(|e| ApiError::IoError {
            message: format!("Failed to read from stdout: {e}"),
        })?;

        let line = line.ok_or_else(|| ApiError::ProtocolError {
            message: "Adapter stdout closed unexpectedly during handshake".to_string(),
        })?;

        // Try to parse as JSON
        let json: serde_json::Value =
            match serde_json::from_str(&line) {
                Ok(j) => j,
                Err(_) => {
                    // Not JSON, likely debug output from adapter
                    log::debug!("[handshake stdout] {line}");
                    continue;
                }
            };

        // Check if this is the response we're waiting for
        if let Some(id) = json.get("id") {
            let id_str = match id {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => continue,
            };

            if id_str == expected_id {
                return Ok(json);
            }

            // Different response ID - log and continue
            log::debug!("[handshake] Unexpected response id={id_str}, waiting for {expected_id}");
        } else {
            // No ID - probably a notification, log and continue
            let method = json.get("method").and_then(|m| m.as_str()).unwrap_or("unknown");
            log::debug!("[handshake] Received notification: method={method}");
        }
    }
}
