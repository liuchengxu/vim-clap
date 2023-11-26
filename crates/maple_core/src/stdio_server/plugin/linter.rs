use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::plugin::{ActionRequest, ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::{Vim, VimResult};
use ide::linting::{Diagnostic, DiagnosticSpan};
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::cmp::Ordering as CmpOrdering;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::task::JoinHandle;

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
struct BufferDiagnostics {
    /// Whether the diagnostics have been refreshed.
    refreshed: Arc<AtomicBool>,
    /// List of diagnostics, in sorted manner.
    inner: Arc<RwLock<Vec<Diagnostic>>>,
}

impl Serialize for BufferDiagnostics {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.inner.read().serialize(serializer)
    }
}

impl BufferDiagnostics {
    /// Append new diagnostics and returns the count of latest diagnostics.
    fn append(&self, new_diagnostics: Vec<Diagnostic>) -> Count {
        let mut diagnostics = self.inner.write();
        diagnostics.extend(new_diagnostics);

        diagnostics.sort_by(|a, b| a.spans[0].line_start.cmp(&b.spans[0].line_start));

        let mut count = Count::default();
        for d in diagnostics.iter() {
            if d.is_error() {
                count.error += 1;
            } else if d.is_warn() {
                count.warn += 1;
            }
        }

        count
    }

    /// Clear the diagnostics list.
    fn reset(&self) {
        self.refreshed.store(false, Ordering::SeqCst);
        let mut diagnostics = self.inner.write();
        diagnostics.clear();
    }

    /// Returns a tuple of (line_number, column_start) of the sibling diagnostic.
    fn find_sibling(
        &self,
        from_line_number: usize,
        kind: DiagnosticKind,
        direction: Direction,
    ) -> Option<(usize, usize)> {
        use CmpOrdering::{Greater, Less};
        use DiagnosticKind::{Error, Warn};
        use Direction::{First, Last, Next, Prev};

        let diagnostics = self.inner.read();

        let errors = || {
            diagnostics
                .iter()
                .filter_map(|d| if d.is_error() { d.spans.get(0) } else { None })
        };

        let warnings = || {
            diagnostics
                .iter()
                .filter_map(|d| if d.is_warn() { d.spans.get(0) } else { None })
        };

        let check_span = |span: &DiagnosticSpan, ordering: CmpOrdering| {
            if span.line_start.cmp(&from_line_number) == ordering {
                Some(span.start_pos())
            } else {
                None
            }
        };

        match (kind, direction) {
            (Error, First) => errors().next().map(|span| span.start_pos()),
            (Error, Last) => errors().last().map(|span| span.start_pos()),
            (Error, Next) => errors().find_map(|span| check_span(span, Greater)),
            (Error, Prev) => errors().rev().find_map(|span| check_span(span, Less)),
            (Warn, First) => warnings().next().map(|span| span.start_pos()),
            (Warn, Last) => warnings().last().map(|span| span.start_pos()),
            (Warn, Next) => warnings().find_map(|span| check_span(span, Greater)),
            (Warn, Prev) => warnings().rev().find_map(|span| check_span(span, Less)),
        }
    }

    async fn display_diagnostics_at_cursor(&self, vim: &Vim) -> VimResult<()> {
        let lnum = vim.line(".").await?;
        let col = vim.col(".").await?;

        let diagnostics = self.inner.read();

        let current_diagnostics = diagnostics
            .iter()
            .filter(|d| d.spans.iter().any(|span| span.line_start == lnum))
            .collect::<Vec<_>>();

        if current_diagnostics.is_empty() {
            vim.bare_exec("clap#plugin#linter#close_top_right")?;
        } else {
            let diagnostic_at_cursor = current_diagnostics
                .iter()
                .filter(|d| {
                    d.spans
                        .iter()
                        .any(|span| col >= span.column_start && col < span.column_end)
                })
                .collect::<Vec<_>>();

            // Display the specific diagnostic if the cursor is on it,
            // otherwise display all the diagnostics in this line.
            if diagnostic_at_cursor.is_empty() {
                vim.exec(
                    "clap#plugin#linter#display_top_right",
                    [current_diagnostics],
                )?;
            } else {
                vim.exec(
                    "clap#plugin#linter#display_top_right",
                    [diagnostic_at_cursor],
                )?;
            }
        }

        Ok(())
    }
}

#[derive(Clone)]
struct LinterResultHandler {
    bufnr: usize,
    vim: Vim,
    buffer_diagnostics: BufferDiagnostics,
}

impl LinterResultHandler {
    fn new(bufnr: usize, vim: Vim, buffer_diagnostics: BufferDiagnostics) -> Self {
        Self {
            bufnr,
            vim,
            buffer_diagnostics,
        }
    }
}

impl ide::linting::HandleLinterResult for LinterResultHandler {
    fn handle_linter_result(
        &self,
        linter_result: ide::linting::LinterResult,
    ) -> std::io::Result<()> {
        let mut new_diagnostics = linter_result.diagnostics;
        new_diagnostics.sort_by(|a, b| a.spans[0].line_start.cmp(&b.spans[0].line_start));
        new_diagnostics.dedup();

        let first_lint_result_arrives = self
            .buffer_diagnostics
            .refreshed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok();

        let new_count = if first_lint_result_arrives {
            let _ = self.vim.exec(
                "clap#plugin#linter#refresh_highlights",
                (self.bufnr, &new_diagnostics),
            );

            self.buffer_diagnostics.append(new_diagnostics)
        } else {
            // Remove the potential duplicated results from multiple linters.
            let existing = self.buffer_diagnostics.inner.read();
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

            self.buffer_diagnostics.append(followup_diagnostics)
        };

        let _ = self
            .vim
            .setbufvar(self.bufnr, "clap_diagnostics", new_count);

        let buffer_diagnostics = self.buffer_diagnostics.clone();
        let vim = self.vim.clone();
        tokio::spawn(async move {
            let _ = buffer_diagnostics.display_diagnostics_at_cursor(&vim).await;
        });

        Ok(())
    }
}

type LinterJob = JoinHandle<()>;

#[derive(Debug, Clone)]
struct BufferLinterInfo {
    filetype: String,
    workspace: PathBuf,
    source_file: PathBuf,
    diagnostics: BufferDiagnostics,
    current_jobs: Arc<Mutex<Vec<LinterJob>>>,
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
            current_jobs: Arc::new(Mutex::new(Vec::new())),
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

        let mut current_jobs = buf_linter_info.current_jobs.lock();

        if !current_jobs.is_empty() {
            for job in current_jobs.drain(..) {
                job.abort();
            }
        }

        let new_jobs = ide::linting::lint_in_background(
            &buf_linter_info.filetype,
            buf_linter_info.source_file.clone(),
            &buf_linter_info.workspace,
            LinterResultHandler::new(bufnr, self.vim.clone(), buf_linter_info.diagnostics.clone()),
        );

        if !new_jobs.is_empty() {
            *current_jobs = new_jobs;
        }
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
            }
            LinterAction::FirstError => {
                self.navigate_diagnostics(DiagnosticKind::Error, Direction::First)
                    .await?;
            }
            LinterAction::LastError => {
                self.navigate_diagnostics(DiagnosticKind::Error, Direction::Last)
                    .await?;
            }
            LinterAction::NextError => {
                self.navigate_diagnostics(DiagnosticKind::Error, Direction::Next)
                    .await?;
            }
            LinterAction::PrevError => {
                self.navigate_diagnostics(DiagnosticKind::Error, Direction::Prev)
                    .await?;
            }
            LinterAction::FirstWarn => {
                self.navigate_diagnostics(DiagnosticKind::Warn, Direction::First)
                    .await?;
            }
            LinterAction::LastWarn => {
                self.navigate_diagnostics(DiagnosticKind::Warn, Direction::Last)
                    .await?;
            }
            LinterAction::NextWarn => {
                self.navigate_diagnostics(DiagnosticKind::Warn, Direction::Next)
                    .await?;
            }
            LinterAction::PrevWarn => {
                self.navigate_diagnostics(DiagnosticKind::Warn, Direction::Prev)
                    .await?;
            }
        }

        Ok(())
    }
}
