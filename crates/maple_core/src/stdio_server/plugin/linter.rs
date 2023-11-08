use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::plugin::{ActionRequest, ClapPlugin, Toggle};
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use linter::Diagnostic;
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::task::JoinHandle;

#[derive(Debug, Clone)]
struct ShareableDiagnostics {
    refreshed: Arc<AtomicBool>,
    inner: Arc<RwLock<Vec<Diagnostic>>>,
}

impl Serialize for ShareableDiagnostics {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.inner.read().serialize(serializer)
    }
}

impl ShareableDiagnostics {
    fn extend(&self, new: Vec<Diagnostic>) {
        let mut diagnostics = self.inner.write();
        diagnostics.extend(new);
    }

    fn reset(&self) {
        self.refreshed.store(false, Ordering::SeqCst);
        let mut diagnostics = self.inner.write();
        diagnostics.clear();
    }
}

#[derive(Clone)]
struct LinterResultHandler {
    bufnr: usize,
    vim: Vim,
    shareable_diagnostics: ShareableDiagnostics,
}

impl LinterResultHandler {
    fn new(bufnr: usize, vim: Vim, shareable_diagnostics: ShareableDiagnostics) -> Self {
        Self {
            bufnr,
            vim,
            shareable_diagnostics,
        }
    }
}

impl linter::HandleLinterResult for LinterResultHandler {
    fn handle_linter_result(&self, linter_result: linter::LinterResult) -> std::io::Result<()> {
        let mut new_diagnostics = linter_result.diagnostics;
        new_diagnostics.sort_by(|a, b| a.spans[0].line_start.cmp(&b.spans[0].line_start));
        new_diagnostics.dedup();

        let first_lint_result_arrives = self
            .shareable_diagnostics
            .refreshed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok();

        if first_lint_result_arrives {
            let _ = self.vim.exec(
                "clap#plugin#linter#refresh_highlights",
                (self.bufnr, &new_diagnostics),
            );

            self.shareable_diagnostics.extend(new_diagnostics);
        } else {
            // Remove the potential duplicated results from multiple linters.
            let existing = self.shareable_diagnostics.inner.read();
            let mut followup_diagnostics = new_diagnostics
                .into_iter()
                .filter(|d| !existing.contains(d))
                .collect::<Vec<_>>();

            followup_diagnostics.dedup();

            // Must drop the lock otherwise the deadlock occurs as
            // the write lock will be acquired later.
            drop(existing);

            if !followup_diagnostics.is_empty() {
                let _ = self.vim.exec(
                    "clap#plugin#linter#add_highlights",
                    (self.bufnr, &followup_diagnostics),
                );
            }

            self.shareable_diagnostics.extend(followup_diagnostics);
        }

        Ok(())
    }
}

type LinterJob = JoinHandle<()>;

#[derive(Debug, Clone)]
struct BufferLinterInfo {
    filetype: String,
    workspace: PathBuf,
    source_file: PathBuf,
    diagnostics: ShareableDiagnostics,
    current_jobs: Arc<Mutex<Vec<LinterJob>>>,
}

impl BufferLinterInfo {
    fn new(filetype: String, workspace: PathBuf, source_file: PathBuf) -> Self {
        Self {
            filetype,
            workspace,
            source_file,
            diagnostics: ShareableDiagnostics {
                refreshed: Arc::new(AtomicBool::new(false)),
                inner: Arc::new(RwLock::new(Vec::new())),
            },
            current_jobs: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(id = "linter", actions = ["lint", "debug", "toggle"])]
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

    async fn on_buf_enter(&mut self, bufnr: usize) -> Result<()> {
        let source_file = self.vim.bufabspath(bufnr).await?;
        let source_file = PathBuf::from(source_file);

        let filetype = self.vim.getbufvar::<String>(bufnr, "&filetype").await?;

        let Some(workspace) = linter::find_workspace(&filetype, &source_file) else {
            return Ok(());
        };

        let buf_linter_info = BufferLinterInfo::new(filetype, workspace.to_path_buf(), source_file);
        self.lint_buffer(bufnr, &buf_linter_info)?;
        self.bufs.insert(bufnr, buf_linter_info);

        Ok(())
    }

    fn lint_buffer(&self, bufnr: usize, buf_linter_info: &BufferLinterInfo) -> Result<()> {
        buf_linter_info.diagnostics.reset();

        let mut current_jobs = buf_linter_info.current_jobs.lock();

        if !current_jobs.is_empty() {
            for job in current_jobs.drain(..) {
                job.abort();
            }
        }

        let new_jobs = linter::lint_in_background(
            &buf_linter_info.filetype,
            buf_linter_info.source_file.clone(),
            &buf_linter_info.workspace,
            LinterResultHandler::new(bufnr, self.vim.clone(), buf_linter_info.diagnostics.clone()),
        );

        if !new_jobs.is_empty() {
            *current_jobs = new_jobs;
        }

        Ok(())
    }

    async fn on_cursor_moved(&self, bufnr: usize) -> Result<()> {
        if let Some(buf_linter_info) = self.bufs.get(&bufnr) {
            let lnum = self.vim.line(".").await?;
            let col = self.vim.col(".").await?;

            let diagnostics = buf_linter_info.diagnostics.inner.read();

            let current_diagnostics = diagnostics
                .iter()
                .filter(|d| d.spans.iter().any(|span| span.line_start == lnum))
                .collect::<Vec<_>>();

            if current_diagnostics.is_empty() {
                self.vim.bare_exec("clap#plugin#linter#clear_top_right")?;
            } else {
                let diagnostic_at_cursor = current_diagnostics
                    .iter()
                    .filter(|d| {
                        d.spans
                            .iter()
                            .any(|span| col >= span.column_start && col < span.column_end)
                    })
                    .collect::<Vec<_>>();

                // Display the specific diagnostic if the cursor is on it, otherwise display all
                // the diagnostics in this line.
                if diagnostic_at_cursor.is_empty() {
                    self.vim.exec(
                        "clap#plugin#linter#display_top_right",
                        [current_diagnostics],
                    )?;
                } else {
                    self.vim.exec(
                        "clap#plugin#linter#display_top_right",
                        [diagnostic_at_cursor],
                    )?;
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapPlugin for Linter {
    #[maple_derive::subscriptions]
    async fn handle_autocmd(&mut self, autocmd: AutocmdEvent) -> Result<()> {
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
                    self.lint_buffer(bufnr, buf_linter_info)?;
                }
            }
            BufDelete => {
                self.bufs.remove(&bufnr);
            }
            CursorMoved => {
                self.on_cursor_moved(bufnr).await?;
            }
            event => {
                return Err(anyhow::anyhow!(
                    "Unhandled {event:?}, incomplete subscriptions?",
                ))
            }
        }

        Ok(())
    }

    async fn handle_action(&mut self, action: ActionRequest) -> Result<()> {
        let ActionRequest { method, params: _ } = action;
        match self.parse_action(method)? {
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
        }

        Ok(())
    }
}
