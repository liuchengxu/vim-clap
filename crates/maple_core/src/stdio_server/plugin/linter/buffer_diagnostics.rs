use crate::stdio_server::vim::{Vim, VimResult};
use ide::linting::LinterDiagnostics;
use ide::linting::{Diagnostic, DiagnosticSpan};
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::cmp::Ordering as CmpOrdering;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

use super::{Count, DiagnosticKind, Direction};

#[derive(Debug, Clone)]
pub struct BufferDiagnostics {
    /// Whether the diagnostics have been refreshed.
    pub refreshed: Arc<AtomicBool>,

    /// List of sorted diagnostics.
    pub inner: Arc<RwLock<Vec<Diagnostic>>>,
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
    pub fn append(&self, new_diagnostics: Vec<Diagnostic>) -> Count {
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
    pub fn reset(&self) {
        self.refreshed.store(false, Ordering::SeqCst);
        let mut diagnostics = self.inner.write();
        diagnostics.clear();
    }

    /// Returns a tuple of (line_number, column_start) of the sibling diagnostic.
    pub fn find_sibling(
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
                .filter_map(|d| if d.is_error() { d.spans.first() } else { None })
        };

        let warnings = || {
            diagnostics
                .iter()
                .filter_map(|d| if d.is_warn() { d.spans.first() } else { None })
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

    pub async fn display_diagnostics_at_cursor(&self, vim: &Vim) -> VimResult<()> {
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
pub struct LinterDiagnosticsHandler {
    bufnr: usize,
    vim: Vim,
    buffer_diagnostics: BufferDiagnostics,
}

impl LinterDiagnosticsHandler {
    pub fn new(bufnr: usize, vim: Vim, buffer_diagnostics: BufferDiagnostics) -> Self {
        Self {
            bufnr,
            vim,
            buffer_diagnostics,
        }
    }

    pub fn on_linter_diagnostics(
        &self,
        linter_diagnostics: ide::linting::LinterDiagnostics,
    ) -> std::io::Result<()> {
        let mut new_diagnostics = linter_diagnostics.diagnostics;
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

/*
enum WorkerMessage {
    StartBufferLinting,
    PublishLinterDiagnostics(LinterDiagnostics),
    PublishLspDiagnostics,
}

pub fn start_buffer_diagnostics_worker() -> UnboundedSender<WorkerMessage> {
    let (worker_msg_sender, worker_msg_receiver) = unbounded_channel();

    tokio::spawn(async move {
        let worker = BufferDiagnosticsWorker {
            worker_msg_receiver,
        };

        worker.run().await;
    });

    worker_msg_sender
}

// Merge the diagnostics reported from LSP and other external tools.
struct BufferDiagnosticsWorker {
    worker_msg_receiver: UnboundedReceiver<WorkerMessage>,
    buffer_diagnostics: HashMap<usize, BufferDiagnostics>,
}

impl BufferDiagnosticsWorker {
    async fn run(mut self) {
        while let Some(worker_msg) = self.worker_msg_receiver.recv().await {
            match worker_msg {
                WorkerMessage::StartBufferLinting => {}
                WorkerMessage::PublishLinterDiagnostics(linter_diagnostics) => {}
                WorkerMessage::PublishLspDiagnostics => {}
            }
        }
    }
}
*/
