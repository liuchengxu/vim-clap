//! Embedded PTY terminal commands.

use std::io::Write;
use std::sync::Mutex;

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use tauri::ipc::Channel;
use tokio::task::JoinHandle;

use crate::state::AppState;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

/// Events streamed from the terminal to the frontend via a Tauri Channel.
#[derive(Clone, serde::Serialize)]
#[serde(tag = "event", content = "data")]
pub enum TerminalEvent {
    /// Raw output bytes from the PTY.
    Output(Vec<u8>),
    /// Process exited with optional exit code.
    Exit { code: Option<u32> },
}

/// A live PTY session.
struct TerminalSession {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    read_task: JoinHandle<()>,
    reap_task: JoinHandle<()>,
}

/// State holding the active terminal session (at most one).
#[derive(Default)]
pub struct TerminalState {
    session: Mutex<Option<TerminalSession>>,
}

/// Kill and clean up an existing session, if any.
fn kill_session(session: &mut Option<TerminalSession>) {
    if let Some(mut sess) = session.take() {
        let _ = sess.killer.kill();
        sess.read_task.abort();
        sess.reap_task.abort();
    }
}

/// Spawn a new terminal session.
///
/// Kills any existing session first. The shell process inherits the
/// working directory of the currently open file (or the home directory).
/// Terminal output and exit events are streamed to the frontend via
/// the provided `on_event` channel.
#[tauri::command]
pub async fn spawn_terminal(
    rows: u16,
    cols: u16,
    on_event: Channel<TerminalEvent>,
    state: State<'_, Arc<RwLock<AppState>>>,
    terminal_state: State<'_, TerminalState>,
) -> Result<(), String> {
    // Kill existing session
    {
        let mut guard = terminal_state
            .session
            .lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        kill_session(&mut guard);
    }

    // Determine shell
    let shell = if cfg!(windows) {
        std::env::var("SHELL").unwrap_or_else(|_| "powershell.exe".to_string())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    };

    // Determine working directory from currently open file
    let cwd = {
        let app_state = state.read().await;
        app_state
            .current_file
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .filter(|d| d.exists())
    }
    .or_else(dirs::home_dir)
    .unwrap_or_else(std::env::temp_dir);

    // Open PTY
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {e}"))?;

    // Build command
    let mut cmd = CommandBuilder::new(&shell);
    cmd.cwd(&cwd);
    cmd.env("TERM", "xterm-256color");

    // Spawn child
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {e}"))?;

    let killer = child.clone_killer();
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {e}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take PTY writer: {e}"))?;

    // Spawn read task — streams PTY output to frontend
    let event_channel = on_event.clone();
    let read_task = tokio::task::spawn_blocking(move || {
        use std::io::Read;
        let mut reader = reader;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = event_channel.send(TerminalEvent::Output(buf[..n].to_vec()));
                }
                Err(_) => break,
            }
        }
    });

    // Spawn reap task — waits for child exit and sends exit event
    let exit_channel = on_event;
    let reap_task = tokio::task::spawn_blocking(move || {
        let status = child.wait();
        let code = status.ok().map(|s| s.exit_code());
        let _ = exit_channel.send(TerminalEvent::Exit { code });
    });

    // Store session
    {
        let mut guard = terminal_state
            .session
            .lock()
            .map_err(|e| format!("Lock poisoned: {e}"))?;
        *guard = Some(TerminalSession {
            writer,
            master: pair.master,
            killer,
            read_task,
            reap_task,
        });
    }

    tracing::info!(shell = %shell, cwd = %cwd.display(), "Spawned terminal session");
    Ok(())
}

/// Write data (keystrokes) to the terminal.
#[tauri::command]
pub fn write_terminal(
    data: String,
    terminal_state: State<'_, TerminalState>,
) -> Result<(), String> {
    let mut guard = terminal_state
        .session
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    let session = guard.as_mut().ok_or("No active terminal session")?;
    session
        .writer
        .write_all(data.as_bytes())
        .map_err(|e| format!("Failed to write to terminal: {e}"))?;
    session
        .writer
        .flush()
        .map_err(|e| format!("Failed to flush terminal: {e}"))?;
    Ok(())
}

/// Resize the terminal.
#[tauri::command]
pub fn resize_terminal(
    rows: u16,
    cols: u16,
    terminal_state: State<'_, TerminalState>,
) -> Result<(), String> {
    let guard = terminal_state
        .session
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    let session = guard.as_ref().ok_or("No active terminal session")?;
    session
        .master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to resize terminal: {e}"))?;
    Ok(())
}

/// Kill the active terminal session (idempotent).
#[tauri::command]
pub fn kill_terminal(terminal_state: State<'_, TerminalState>) -> Result<(), String> {
    let mut guard = terminal_state
        .session
        .lock()
        .map_err(|e| format!("Lock poisoned: {e}"))?;
    kill_session(&mut guard);
    tracing::info!("Killed terminal session");
    Ok(())
}
