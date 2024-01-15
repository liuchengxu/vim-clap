mod buffer_diagnostics;

use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::plugin::{ActionRequest, ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::{Vim, VimResult};
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::task::JoinHandle;

use self::buffer_diagnostics::{BufferDiagnostics, BufferDiagnosticsHandler};

#[derive(Debug, Default, Clone, Serialize)]
struct Count {
    error: usize,
    warn: usize,
}

enum Direction {
    First,
    Last,
    Next,
    Prev,
}

enum DiagnosticKind {
    Error,
    Warn,
}

#[derive(Debug, Clone)]
struct BufferLinterInfo {
    filetype: String,
    workspace: PathBuf,
    source_file: PathBuf,
    diagnostics: BufferDiagnostics,
}

impl BufferLinterInfo {
    fn new(filetype: String, workspace: PathBuf, source_file: PathBuf) -> Self {
        Self {
            filetype,
            workspace,
            source_file,
            diagnostics: BufferDiagnostics {
                refreshed: Arc::new(AtomicBool::new(false)),
                inner: Arc::new(RwLock::new(Vec::new())),
            },
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
    bufs: HashMap<usize, BufferLinterInfo>,
    toggle: Toggle,
}

impl Linter {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            bufs: HashMap::new(),
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

        let buf_linter_info = BufferLinterInfo::new(filetype, workspace.to_path_buf(), source_file);
        self.lint_buffer(bufnr, &buf_linter_info);
        self.bufs.insert(bufnr, buf_linter_info);

        Ok(())
    }

    fn lint_buffer(&self, bufnr: usize, buf_linter_info: &BufferLinterInfo) {
        buf_linter_info.diagnostics.reset();

        let (diagnostics_sender, mut diagnostics_receiver) = tokio::sync::mpsc::unbounded_channel();

        let buf_linter_info = buf_linter_info.clone();
        tokio::spawn(async move {
            ide::linting::start_linting_in_background(
                &buf_linter_info.filetype,
                buf_linter_info.source_file.clone(),
                &buf_linter_info.workspace,
                diagnostics_sender,
            )
            .await;
        });

        tokio::spawn({
            let buffer_diagnostics_handler = BufferDiagnosticsHandler::new(
                bufnr,
                self.vim.clone(),
                buf_linter_info.diagnostics.clone(),
            );

            async move {
                while let Some(linter_diagnostics) = diagnostics_receiver.recv().await {
                    let _ = buffer_diagnostics_handler.process_diagnostics(linter_diagnostics);
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
        if let Some(buf_linter_info) = self.bufs.get(&bufnr) {
            let lnum = self.vim.line(".").await?;
            if let Some((lnum, col)) = buf_linter_info
                .diagnostics
                .find_sibling(lnum, kind, direction)
            {
                self.vim.exec("cursor", [lnum, col])?;
                self.vim.exec("execute", "normal! zz")?;
            }
        }
        Ok(())
    }

    async fn on_cursor_moved(&self, bufnr: usize) -> VimResult<()> {
        if let Some(buf_linter_info) = self.bufs.get(&bufnr) {
            buf_linter_info
                .diagnostics
                .display_diagnostics_at_cursor(&self.vim)
                .await?;
        }

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
                if let Some(buf_linter_info) = self.bufs.get(&bufnr) {
                    self.lint_buffer(bufnr, buf_linter_info);
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

                if let Some(buf_linter_info) = self.bufs.get(&bufnr) {
                    let lnum = self.vim.line(".").await?;
                    let diagnostics = buf_linter_info.diagnostics.inner.read();
                    let current_diagnostics = diagnostics
                        .iter()
                        .filter(|d| d.spans.iter().any(|span| span.line_start == lnum))
                        .collect::<Vec<_>>();

                    for diagnostic in current_diagnostics {
                        self.vim.echo_info(diagnostic.human_message())?;
                    }

                    return Ok(());
                }

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
