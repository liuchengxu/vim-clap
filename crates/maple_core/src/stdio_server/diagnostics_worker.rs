use crate::stdio_server::plugin::PluginResult;
use crate::stdio_server::vim::{Vim, VimResult};
use crate::types::{DiagnosticKind, Direction};
use code_tools::linting::{Code, Diagnostic, DiagnosticSpan, LinterDiagnostics, Severity};
use parking_lot::RwLock;
use serde::Serialize;
use std::cmp::Ordering as CmpOrdering;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

#[derive(Default, Serialize)]
struct Stats {
    error: usize,
    warn: usize,
    hint: usize,
}

#[derive(Debug, Clone)]
struct BufferDiagnostics {
    /// This flag indicates whether the received results are
    /// the first received ones, for having multiple diagnostics
    /// sources is very likely,
    first_result_arrived: Arc<AtomicBool>,

    /// List of sorted diagnostics.
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
    /// Constructs a new instance of [`BufferDiagnostics`].
    fn new() -> Self {
        Self {
            first_result_arrived: Arc::new(false.into()),
            inner: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Append new diagnostics and returns the count of latest diagnostics.
    fn append(&self, new_diagnostics: Vec<Diagnostic>) -> Stats {
        let mut diagnostics = self.inner.write();
        diagnostics.extend(new_diagnostics);

        diagnostics.sort_by(|a, b| a.spans[0].line_start.cmp(&b.spans[0].line_start));

        let mut stats = Stats::default();
        for d in diagnostics.iter() {
            if d.is_error() {
                stats.error += 1;
            } else if d.is_warn() {
                stats.warn += 1;
            } else if d.is_hint() {
                stats.hint += 1;
            }
        }

        stats
    }

    /// Clear the diagnostics list.
    fn reset(&self) {
        self.first_result_arrived.store(false, Ordering::SeqCst);
        let mut diagnostics = self.inner.write();
        diagnostics.clear();
    }

    /// Returns a tuple of (line_number, column_start) of the sibling diagnostic.
    fn find_sibling_position(
        &self,
        from_line_number: usize,
        kind: DiagnosticKind,
        direction: Direction,
    ) -> Option<(usize, usize)> {
        use CmpOrdering::{Greater, Less};
        use DiagnosticKind::{All, Error, Hint, Warn};
        use Direction::{First, Last, Next, Prev};

        let diagnostics = self.inner.read();

        let errors = || {
            diagnostics
                .iter()
                .filter_map(|d| if d.is_error() { d.spans.first() } else { None })
        };

        let warnings = || {
            diagnostics
                .iter()
                .filter_map(|d| if d.is_warn() { d.spans.first() } else { None })
        };

        let hints = || {
            diagnostics
                .iter()
                .filter_map(|d| if d.is_hint() { d.spans.first() } else { None })
        };

        let check_span = |span: &DiagnosticSpan, ordering: CmpOrdering| {
            if span.line_start.cmp(&from_line_number) == ordering {
                Some(span.start_pos())
            } else {
                None
            }
        };

        let spans = || diagnostics.iter().filter_map(|d| d.spans.first());

        match (kind, direction) {
            (All, First) => spans().next().map(|span| span.start_pos()),
            (All, Last) => spans().last().map(|span| span.start_pos()),
            (All, Next) => spans().find_map(|span| check_span(span, Greater)),
            (All, Prev) => spans().rev().find_map(|span| check_span(span, Less)),
            (Error, First) => errors().next().map(|span| span.start_pos()),
            (Error, Last) => errors().last().map(|span| span.start_pos()),
            (Error, Next) => errors().find_map(|span| check_span(span, Greater)),
            (Error, Prev) => errors().rev().find_map(|span| check_span(span, Less)),
            (Warn, First) => warnings().next().map(|span| span.start_pos()),
            (Warn, Last) => warnings().last().map(|span| span.start_pos()),
            (Warn, Next) => warnings().find_map(|span| check_span(span, Greater)),
            (Warn, Prev) => warnings().rev().find_map(|span| check_span(span, Less)),
            (Hint, First) => hints().next().map(|span| span.start_pos()),
            (Hint, Last) => hints().last().map(|span| span.start_pos()),
            (Hint, Next) => hints().find_map(|span| check_span(span, Greater)),
            (Hint, Prev) => hints().rev().find_map(|span| check_span(span, Less)),
        }
    }

    async fn display_diagnostics_under_cursor(&self, vim: &Vim) -> VimResult<()> {
        let lnum = vim.line(".").await?;
        let col = vim.col(".").await?;

        let diagnostics = self.inner.read();

        let current_diagnostics = diagnostics
            .iter()
            .filter(|d| d.spans.iter().any(|span| span.line_start == lnum))
            .collect::<Vec<_>>();

        if current_diagnostics.is_empty() {
            vim.bare_exec("clap#plugin#diagnostics#close_top_right")?;
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
                    "clap#plugin#diagnostics#display_top_right",
                    [current_diagnostics],
                )?;
            } else {
                vim.exec(
                    "clap#plugin#diagnostics#display_top_right",
                    [diagnostic_at_cursor],
                )?;
            }
        }

        Ok(())
    }
}

fn update_buffer_diagnostics(
    bufnr: usize,
    vim: &Vim,
    buffer_diagnostics: &BufferDiagnostics,
    mut new_diagnostics: Vec<Diagnostic>,
) -> std::io::Result<()> {
    new_diagnostics.sort_by(|a, b| a.spans[0].line_start.cmp(&b.spans[0].line_start));
    new_diagnostics.dedup();

    let is_first_result = buffer_diagnostics
        .first_result_arrived
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();

    let new_stats = if is_first_result {
        let _ = vim.exec(
            "clap#plugin#diagnostics#refresh_highlights",
            (bufnr, &new_diagnostics),
        );

        buffer_diagnostics.append(new_diagnostics)
    } else {
        // Remove the potential duplicated results from multiple diagnostic reporters.
        let existing = buffer_diagnostics.inner.read();
        let mut followup_diagnostics = new_diagnostics
            .into_iter()
            .filter(|d| !existing.contains(d))
            .collect::<Vec<_>>();

        followup_diagnostics.dedup();

        // Must drop the lock otherwise the deadlock occurs as
        // the write lock will be acquired later.
        drop(existing);

        if !followup_diagnostics.is_empty() {
            let _ = vim.exec(
                "clap#plugin#diagnostics#add_highlights",
                (bufnr, &followup_diagnostics),
            );
        }

        buffer_diagnostics.append(followup_diagnostics)
    };

    let _ = vim.setbufvar(bufnr, "clap_diagnostics", new_stats);

    tokio::spawn({
        let buffer_diagnostics = buffer_diagnostics.clone();
        let vim = vim.clone();

        async move {
            let _ = buffer_diagnostics
                .display_diagnostics_under_cursor(&vim)
                .await;
        }
    });

    Ok(())
}

fn convert_lsp_diagnostic_to_diagnostic(lsp_diag: maple_lsp::lsp::Diagnostic) -> Diagnostic {
    use maple_lsp::lsp;

    let severity = lsp_diag
        .severity
        .map(|s| match s {
            lsp::DiagnosticSeverity::ERROR => Severity::Error,
            lsp::DiagnosticSeverity::WARNING => Severity::Warning,
            lsp::DiagnosticSeverity::INFORMATION => Severity::Info,
            lsp::DiagnosticSeverity::HINT => Severity::Hint,
            _ => Severity::Unknown,
        })
        .unwrap_or(Severity::Unknown);

    let code = lsp_diag
        .code
        .map(|c| match c {
            lsp::NumberOrString::Number(n) => n.to_string(),
            lsp::NumberOrString::String(s) => s,
        })
        .unwrap_or_default();

    // Replace "\r\n", "\r", and "\n" with " "
    let message = ["\r\n", "\r", "\n"]
        .iter()
        .fold(lsp_diag.message, |acc, &newline| acc.replace(newline, " "));

    let spans = vec![DiagnosticSpan {
        line_start: lsp_diag.range.start.line as usize + 1,
        line_end: lsp_diag.range.end.line as usize + 1,
        column_start: lsp_diag.range.start.character as usize + 1,
        column_end: lsp_diag.range.end.character as usize + 1,
    }];

    Diagnostic {
        message,
        spans,
        code: Code { code },
        severity,
    }
}

pub enum WorkerMessage {
    ShowDiagnostics(usize),
    ShowDiagnosticsUnderCursor(usize),
    ShowDiagnosticsUnderCursorInFloatWin(usize),
    NavigateDiagnostics((usize, DiagnosticKind, Direction)),
    ResetBufferDiagnostics(usize),
    LinterDiagnostics((usize, LinterDiagnostics)),
    LspDiagnostics(maple_lsp::lsp::PublishDiagnosticsParams),
}

/// A worker running in a separate task, responsible for processing the diagnostics
/// reported from LSP as well as the other external linter tools and performing the
/// actions issued from vim-clap plugins.
struct BufferDiagnosticsWorker {
    vim: Vim,
    worker_msg_receiver: UnboundedReceiver<WorkerMessage>,
    /// State of each buffer's diagnostics.
    buffer_diagnostics: HashMap<usize, BufferDiagnostics>,
}

impl BufferDiagnosticsWorker {
    async fn run(mut self) -> PluginResult<()> {
        while let Some(worker_msg) = self.worker_msg_receiver.recv().await {
            match worker_msg {
                WorkerMessage::ShowDiagnostics(bufnr) => {
                    if let Some(diagnostics) = self.buffer_diagnostics.get(&bufnr) {
                        let diagnostics = diagnostics.inner.read();
                        self.vim.echo_message(format!("{diagnostics:?}"))?;
                    } else {
                        self.vim
                            .echo_message(format!("diagnostics not found for buffer {bufnr}"))?;
                    }
                }
                WorkerMessage::ShowDiagnosticsUnderCursor(bufnr) => {
                    if let Some(diagnostics) = self.buffer_diagnostics.get(&bufnr) {
                        let Ok(lnum) = self.vim.line(".").await else {
                            continue;
                        };

                        let diagnostics = diagnostics.inner.read();
                        let current_diagnostics = diagnostics
                            .iter()
                            .filter(|d| d.spans.iter().any(|span| span.line_start == lnum))
                            .collect::<Vec<_>>();

                        for diagnostic in current_diagnostics {
                            tracing::debug!("cursor_diagnostic: {diagnostic:?}");
                            self.vim.echo_info(diagnostic.human_message())?;
                        }
                    }
                }
                WorkerMessage::NavigateDiagnostics((bufnr, kind, direction)) => {
                    if let Some(diagnostics) = self.buffer_diagnostics.get(&bufnr) {
                        let lnum = self.vim.line(".").await?;
                        if let Some((lnum, col)) =
                            diagnostics.find_sibling_position(lnum, kind, direction)
                        {
                            self.vim.exec("cursor", [lnum, col])?;
                            self.vim.exec("execute", "normal! zz")?;
                        }
                    }
                }
                WorkerMessage::ShowDiagnosticsUnderCursorInFloatWin(bufnr) => {
                    if let Some(diagnostics) = self.buffer_diagnostics.get(&bufnr) {
                        diagnostics
                            .display_diagnostics_under_cursor(&self.vim)
                            .await?;
                    }
                }
                WorkerMessage::ResetBufferDiagnostics(bufnr) => {
                    self.buffer_diagnostics
                        .entry(bufnr)
                        .and_modify(|v| v.reset())
                        .or_insert_with(BufferDiagnostics::new);
                    self.vim
                        .setbufvar(bufnr, "clap_diagnostics", Stats::default())?;
                    self.vim
                        .exec("clap#plugin#diagnostics#toggle_off", [bufnr])?;
                }
                WorkerMessage::LinterDiagnostics((bufnr, linter_diagnostics)) => {
                    tracing::trace!(bufnr, "Recv linter diagnostics: {linter_diagnostics:?}");

                    if let Some(buffer_diagnostics) = self.buffer_diagnostics.get(&bufnr) {
                        update_buffer_diagnostics(
                            bufnr,
                            &self.vim,
                            buffer_diagnostics,
                            linter_diagnostics.diagnostics,
                        )?;
                    }
                }
                WorkerMessage::LspDiagnostics(diagnostics_params) => {
                    let path = diagnostics_params.uri.path();

                    // TODO: uri.path may not be loaded as a buffer.
                    let Ok(bufnr) = self.vim.bufnr(path).await else {
                        continue;
                    };

                    let diagnostics = diagnostics_params
                        .diagnostics
                        .into_iter()
                        .map(convert_lsp_diagnostic_to_diagnostic)
                        .collect::<Vec<_>>();

                    tracing::trace!(path, "Recv LSP diagnostics: {diagnostics:?}");

                    let buffer_diagnostics = self
                        .buffer_diagnostics
                        .entry(bufnr)
                        .or_insert_with(BufferDiagnostics::new);

                    update_buffer_diagnostics(bufnr, &self.vim, buffer_diagnostics, diagnostics)?;
                }
            }
        }

        Ok(())
    }
}

pub fn start_buffer_diagnostics_worker(vim: Vim) -> UnboundedSender<WorkerMessage> {
    let (worker_msg_sender, worker_msg_receiver) = unbounded_channel();

    tokio::spawn(async move {
        let worker = BufferDiagnosticsWorker {
            vim,
            worker_msg_receiver,
            buffer_diagnostics: HashMap::new(),
        };

        if let Err(e) = worker.run().await {
            // Restart the worker?
            tracing::error!(error = ?e, "buffer diagnostics worker exited");
        }
    });

    worker_msg_sender
}
