//! TerminalManager - executes terminal commands scoped to a workspace root.
//!
//! US-08: Spawns a command, streams stdout/stderr, and reports exit status.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot, Mutex};
use uuid::Uuid;

use crate::api::types::{ApiError, TerminalId};

const OUTPUT_CHANNEL_CAPACITY: usize = 128;
const OUTPUT_BUFFER_SIZE: usize = 4096;

/// Handle to a running terminal command.
pub struct TerminalRunHandle {
    pub terminal_id: TerminalId,
    pub stdout_rx: mpsc::Receiver<String>,
    pub stderr_rx: mpsc::Receiver<String>,
    pub exit_rx: oneshot::Receiver<TerminalExit>,
}

/// Exit metadata for a terminal run.
pub struct TerminalExit {
    pub exit_code: Option<i32>,
    pub user_stopped: bool,
}

struct TerminalControl {
    kill_tx: Option<oneshot::Sender<()>>,
}

/// Per-workspace terminal manager.
pub struct TerminalManager {
    workspace_root: PathBuf,
    runs: Arc<Mutex<HashMap<TerminalId, TerminalControl>>>,
}

impl TerminalManager {
    /// Create a new TerminalManager scoped to a workspace root.
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            runs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn a terminal command and stream its output.
    pub async fn spawn_run(&self, command: String) -> Result<TerminalRunHandle, ApiError> {
        if command.trim().is_empty() {
            return Err(ApiError::InvalidInput {
                message: "Command cannot be empty".to_string(),
            });
        }

        let terminal_id = Uuid::new_v4().to_string();

        log::info!(
            "Spawning terminal command: terminal_id={terminal_id}, cwd={}",
            self.workspace_root.display()
        );
        log::debug!(
            "Terminal command received: terminal_id={terminal_id}, len={}",
            command.len()
        );

        let mut cmd = build_shell_command(&command);
        cmd.current_dir(&self.workspace_root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd.spawn().map_err(|e| ApiError::IoError {
            message: format!("Failed to spawn terminal command: {e}"),
        })?;

        let stdout = child.stdout.take().ok_or_else(|| ApiError::IoError {
            message: "Failed to capture stdout".to_string(),
        })?;
        let stderr = child.stderr.take().ok_or_else(|| ApiError::IoError {
            message: "Failed to capture stderr".to_string(),
        })?;

        let (stdout_tx, stdout_rx) = mpsc::channel(OUTPUT_CHANNEL_CAPACITY);
        let (stderr_tx, stderr_rx) = mpsc::channel(OUTPUT_CHANNEL_CAPACITY);
        let (exit_tx, exit_rx) = oneshot::channel();
        let (kill_tx, mut kill_rx) = oneshot::channel::<()>();
        {
            let mut runs = self.runs.lock().await;
            runs.insert(
                terminal_id.clone(),
                TerminalControl {
                    kill_tx: Some(kill_tx),
                },
            );
        }

        tokio::spawn(stream_to_channel(stdout, stdout_tx, "stdout"));
        tokio::spawn(stream_to_channel(stderr, stderr_tx, "stderr"));

        let runs = self.runs.clone();
        let terminal_id_for_task = terminal_id.clone();
        tokio::spawn(async move {
            let mut user_stopped = false;
            let status = tokio::select! {
                status = child.wait() => status,
                _ = &mut kill_rx => {
                    match child.try_wait() {
                        Ok(Some(status)) => Ok(status),
                        Ok(None) => {
                            user_stopped = true;
                            if let Err(e) = child.kill().await {
                                log::warn!("Terminal kill failed: {e}");
                            }
                            child.wait().await
                        }
                        Err(e) => {
                            user_stopped = true;
                            log::warn!("Terminal process try_wait failed: {e}");
                            if let Err(e) = child.kill().await {
                                log::warn!("Terminal kill failed: {e}");
                            }
                            child.wait().await
                        }
                    }
                }
            };

            let exit_code = match status {
                Ok(status) => status.code(),
                Err(e) => {
                    log::warn!("Terminal process wait failed: {e}");
                    None
                }
            };

            let _ = exit_tx.send(TerminalExit {
                exit_code,
                user_stopped,
            });

            let mut runs = runs.lock().await;
            runs.remove(&terminal_id_for_task);
        });

        Ok(TerminalRunHandle {
            terminal_id,
            stdout_rx,
            stderr_rx,
            exit_rx,
        })
    }

    /// Kill a running terminal command by ID.
    pub async fn kill(&self, terminal_id: TerminalId) -> Result<(), ApiError> {
        if terminal_id.trim().is_empty() {
            return Err(ApiError::InvalidInput {
                message: "Terminal ID cannot be empty".to_string(),
            });
        }

        let mut runs = self.runs.lock().await;
        let Some(control) = runs.get_mut(&terminal_id) else {
            log::debug!("Terminal kill ignored: terminal_id={terminal_id} not found");
            return Ok(());
        };

        if let Some(kill_tx) = control.kill_tx.take() {
            let _ = kill_tx.send(());
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn build_shell_command(command: &str) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.arg("/C").arg(command);
    cmd
}

#[cfg(not(target_os = "windows"))]
fn build_shell_command(command: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);
    cmd
}

async fn stream_to_channel<R: AsyncRead + Unpin>(
    reader: R,
    tx: mpsc::Sender<String>,
    label: &'static str,
) {
    let mut reader = BufReader::new(reader);
    let mut buf = vec![0u8; OUTPUT_BUFFER_SIZE];

    loop {
        match reader.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = tx.try_send(chunk);
            }
            Err(e) => {
                log::warn!("Terminal {label} stream read failed: {e}");
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_spawn_run_captures_stdout() {
        let manager = TerminalManager::new(std::env::temp_dir());

        #[cfg(target_os = "windows")]
        let command = "echo hello".to_string();
        #[cfg(not(target_os = "windows"))]
        let command = "printf 'hello'".to_string();

        let mut handle = manager.spawn_run(command).await.unwrap();
        let mut stdout = String::new();

        while let Ok(Some(chunk)) = timeout(Duration::from_secs(1), handle.stdout_rx.recv()).await {
            stdout.push_str(&chunk);
            if stdout.contains("hello") {
                break;
            }
        }

        let exit = timeout(Duration::from_secs(2), handle.exit_rx)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(exit.exit_code, Some(0));
        assert!(!exit.user_stopped);
        assert!(stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_kill_marks_user_stopped() {
        let manager = TerminalManager::new(std::env::temp_dir());

        #[cfg(target_os = "windows")]
        let command = "ping -n 10 127.0.0.1 >NUL".to_string();
        #[cfg(not(target_os = "windows"))]
        let command = "sleep 5".to_string();

        let handle = manager.spawn_run(command).await.unwrap();
        let terminal_id = handle.terminal_id.clone();

        manager.kill(terminal_id).await.unwrap();

        let exit = timeout(Duration::from_secs(5), handle.exit_rx)
            .await
            .unwrap()
            .unwrap();

        assert!(exit.user_stopped);
    }

    #[tokio::test]
    async fn test_kill_unknown_terminal_ok() {
        let manager = TerminalManager::new(std::env::temp_dir());

        let result = manager.kill("unknown-terminal".to_string()).await;

        assert!(result.is_ok());
    }
}
