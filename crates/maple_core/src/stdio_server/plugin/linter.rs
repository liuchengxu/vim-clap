mod buffer_diagnostics;

use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::plugin::{ActionRequest, ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::{Vim, VimResult};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;

use self::buffer_diagnostics::WorkerMessage;

pub use self::buffer_diagnostics::start_buffer_diagnostics_worker;
pub use self::buffer_diagnostics::WorkerMessage as DiagnosticWorkerMessage;

#[derive(Debug, Default, Clone, Serialize)]
struct Count {
    error: usize,
    warn: usize,
}

pub enum Direction {
    First,
    Last,
    Next,
    Prev,
}

pub enum DiagnosticKind {
    Error,
    Warn,
}

#[derive(Debug, Clone)]
struct BufferInfo {
    filetype: String,
    workspace: PathBuf,
    source_file: PathBuf,
}

impl BufferInfo {
    fn new(filetype: String, workspace: PathBuf, source_file: PathBuf) -> Self {
        Self {
            filetype,
            workspace,
            source_file,
        }
    }
}

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(
  id = "linter",
  actions = [
    "lint",
    "format",
    "first-error",
    "last-error",
    "next-error",
    "prev-error",
    "first-warn",
    "last-warn",
    "next-warn",
    "prev-warn",
    "debug",
    "toggle",
  ]
)]
pub struct Linter {
    vim: Vim,
    bufs: HashMap<usize, BufferInfo>,
    diagnostics_worker_msg_sender: UnboundedSender<WorkerMessage>,
    toggle: Toggle,
}

impl Linter {
    pub fn new(vim: Vim, diagnostics_worker_msg_sender: UnboundedSender<WorkerMessage>) -> Self {
        Self {
            vim,
            bufs: HashMap::new(),
            diagnostics_worker_msg_sender,
            toggle: Toggle::On,
        }
    }

    async fn on_buf_enter(&mut self, bufnr: usize) -> VimResult<()> {
        let source_file = self.vim.bufabspath(bufnr).await?;
        let source_file = PathBuf::from(source_file);

        let filetype = self.vim.getbufvar::<String>(bufnr, "&filetype").await?;

        let Some(workspace) = ide::linting::find_workspace(&filetype, &source_file) else {
            return Ok(());
        };

        let buf_info = BufferInfo::new(filetype, workspace.to_path_buf(), source_file);
        self.lint_buffer(bufnr, &buf_info);
        self.bufs.insert(bufnr, buf_info);

        Ok(())
    }

    fn lint_buffer(&self, bufnr: usize, buf_info: &BufferInfo) {
        if self
            .diagnostics_worker_msg_sender
            .send(WorkerMessage::ResetBufferDiagnostics(bufnr))
            .is_err()
        {
            tracing::error!("buffer diagnostics worker exited unexpectedly");
            return;
        }

        let (diagnostics_sender, mut diagnostics_receiver) = tokio::sync::mpsc::unbounded_channel();

        ide::linting::start_linting_in_background(
            buf_info.filetype.clone(),
            buf_info.source_file.clone(),
            buf_info.workspace.clone(),
            diagnostics_sender,
        );

        tokio::spawn({
            let worker_msg_sender = self.diagnostics_worker_msg_sender.clone();

            async move {
                while let Some(linter_diagnostics) = diagnostics_receiver.recv().await {
                    if let Err(err) = worker_msg_sender.send(WorkerMessage::LinterDiagnostics((
                        bufnr,
                        linter_diagnostics,
                    ))) {
                        tracing::error!(?err, "Failed to send diagnostics from linter");
                    }
                }
            }
        });
    }

    async fn format_buffer(&self, bufnr: usize) -> VimResult<()> {
        let source_file = self.vim.bufabspath(bufnr).await?;
        let source_file = PathBuf::from(source_file);

        let filetype = self.vim.getbufvar::<String>(bufnr, "&filetype").await?;

        let Some(workspace) = ide::linting::find_workspace(&filetype, &source_file) else {
            return Ok(());
        };

        let workspace = workspace.to_path_buf();
        let vim = self.vim.clone();
        tokio::spawn(async move {
            if ide::formatting::run_rustfmt(&source_file, &workspace)
                .await
                .is_ok()
            {
                let _ = vim.bare_exec("clap#util#reload_current_file");
            }
        });
        Ok(())
    }

    async fn navigate_diagnostics(
        &self,
        kind: DiagnosticKind,
        direction: Direction,
    ) -> VimResult<()> {
        let bufnr = self.vim.bufnr("").await?;
        let _ = self
            .diagnostics_worker_msg_sender
            .send(WorkerMessage::NavigateDiagnostics((bufnr, kind, direction)));
        Ok(())
    }

    async fn on_cursor_moved(&self, bufnr: usize) -> VimResult<()> {
        let _ = self
            .diagnostics_worker_msg_sender
            .send(WorkerMessage::DisplayDiagnosticsAtCursor(bufnr));

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapPlugin for Linter {
    #[maple_derive::subscriptions]
    async fn handle_autocmd(&mut self, autocmd: AutocmdEvent) -> Result<(), PluginError> {
        use AutocmdEventType::{BufDelete, BufEnter, BufWritePost, CursorMoved};

        if self.toggle.is_off() {
            return Ok(());
        }

        let (autocmd_event_type, params) = autocmd;

        let bufnr = params.parse_bufnr()?;

        match autocmd_event_type {
            BufEnter => self.on_buf_enter(bufnr).await?,
            BufWritePost => {
                if let Some(buf_info) = self.bufs.get(&bufnr) {
                    self.lint_buffer(bufnr, buf_info);
                }
            }
            BufDelete => {
                self.bufs.remove(&bufnr);
            }
            CursorMoved => {
                self.on_cursor_moved(bufnr).await?;
            }
            event => return Err(PluginError::UnhandledEvent(event)),
        }

        Ok(())
    }

    async fn handle_action(&mut self, action: ActionRequest) -> Result<(), PluginError> {
        use DiagnosticKind::{Error, Warn};
        use Direction::{First, Last, Next, Prev};

        let ActionRequest { method, params: _ } = action;
        match self.parse_action(method)? {
            LinterAction::Toggle => {
                match self.toggle {
                    Toggle::On => {
                        for bufnr in self.bufs.keys() {
                            self.vim.exec("clap#plugin#linter#toggle_off", [bufnr])?;
                        }
                    }
                    Toggle::Off => {
                        let bufnr = self.vim.bufnr("").await?;
                        self.on_buf_enter(bufnr).await?;
                    }
                }
                self.toggle.switch();
            }
            LinterAction::Lint => {
                let bufnr = self.vim.bufnr("").await?;

                // TODO: if requested state exists, return early, otherwise continue.
                let _ = self
                    .diagnostics_worker_msg_sender
                    .send(WorkerMessage::EchoDiagnosticsAtCursor(bufnr));

                self.on_buf_enter(bufnr).await?;
            }
            LinterAction::Debug => {
                let bufnr = self.vim.bufnr("").await?;
                self.on_buf_enter(bufnr).await?;
            }
            LinterAction::Format => {
                let bufnr = self.vim.bufnr("").await?;
                self.format_buffer(bufnr).await?;
            }
            LinterAction::FirstError => {
                self.navigate_diagnostics(Error, First).await?;
            }
            LinterAction::LastError => {
                self.navigate_diagnostics(Error, Last).await?;
            }
            LinterAction::NextError => {
                self.navigate_diagnostics(Error, Next).await?;
            }
            LinterAction::PrevError => {
                self.navigate_diagnostics(Error, Prev).await?;
            }
            LinterAction::FirstWarn => {
                self.navigate_diagnostics(Warn, First).await?;
            }
            LinterAction::LastWarn => {
                self.navigate_diagnostics(Warn, Last).await?;
            }
            LinterAction::NextWarn => {
                self.navigate_diagnostics(Warn, Next).await?;
            }
            LinterAction::PrevWarn => {
                self.navigate_diagnostics(Warn, Prev).await?;
            }
        }

        Ok(())
    }
}
