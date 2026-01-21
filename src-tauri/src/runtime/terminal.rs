//! TerminalManager - executes terminal commands scoped to a workspace root.
//!
//! US-08: Spawns a command, streams stdout/stderr, and reports exit status.

use std::path::PathBuf;
use std::process::Stdio;

use tokio::io::{AsyncRead, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::api::types::{ApiError, TerminalId};

const OUTPUT_CHANNEL_CAPACITY: usize = 128;
const OUTPUT_BUFFER_SIZE: usize = 4096;

/// Handle to a running terminal command.
pub struct TerminalRunHandle {
    pub terminal_id: TerminalId,
    pub stdout_rx: mpsc::Receiver<String>,
    pub stderr_rx: mpsc::Receiver<String>,
    pub exit_rx: oneshot::Receiver<Option<i32>>,
}

/// Per-workspace terminal manager.
pub struct TerminalManager {
    workspace_root: PathBuf,
}

impl TerminalManager {
    /// Create a new TerminalManager scoped to a workspace root.
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
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

        tokio::spawn(stream_to_channel(stdout, stdout_tx, "stdout"));
        tokio::spawn(stream_to_channel(stderr, stderr_tx, "stderr"));

        tokio::spawn(async move {
            let exit_code = match child.wait().await {
                Ok(status) => status.code(),
                Err(e) => {
                    log::warn!("Terminal process wait failed: {e}");
                    None
                }
            };
            let _ = exit_tx.send(exit_code);
        });

        Ok(TerminalRunHandle {
            terminal_id,
            stdout_rx,
            stderr_rx,
            exit_rx,
        })
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
                if tx.send(chunk).await.is_err() {
                    break;
                }
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

        let exit_code = timeout(Duration::from_secs(2), handle.exit_rx)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(exit_code, Some(0));
        assert!(stdout.contains("hello"));
    }
}
