use crate::stdio_server::diagnostics_worker::WorkerMessage;
use crate::stdio_server::plugin::{ClapPlugin, PluginAction, PluginError};
use crate::stdio_server::vim::{Vim, VimResult};
use crate::types::{DiagnosticKind, Direction};
use tokio::sync::mpsc::UnboundedSender;

/// This plugin itself does not do any actual work, it is intended to provide the interface
/// for the diagnostics collectively provided by the linter and lsp plugin.
#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(
  id = "diagnostics",
  actions = [
    // Show the diagnostics in the current buffer.
    "buffer",
    // Show the diagnostics in the cursor line.
    "cursor",
    "first",
    "last",
    "next",
    "prev",
    "firstError",
    "lastError",
    "nextError",
    "prevError",
    "firstWarn",
    "lastWarn",
    "nextWarn",
    "prevWarn",
    "firstHint",
    "lastHint",
    "nextHint",
    "prevHint",
  ]
)]
pub struct Diagnostics {
    vim: Vim,
    diagnostics_worker_msg_sender: UnboundedSender<WorkerMessage>,
}

impl Diagnostics {
    pub fn new(vim: Vim, diagnostics_worker_msg_sender: UnboundedSender<WorkerMessage>) -> Self {
        Self {
            vim,
            diagnostics_worker_msg_sender,
        }
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
}

#[async_trait::async_trait]
impl ClapPlugin for Diagnostics {
    async fn handle_action(&mut self, action: PluginAction) -> Result<(), PluginError> {
        use DiagnosticKind::{All, Error, Hint, Warn};
        use Direction::{First, Last, Next, Prev};

        let PluginAction { method, params: _ } = action;

        match self.parse_action(method)? {
            DiagnosticsAction::Buffer => {
                let bufnr = self.vim.bufnr("").await?;
                let _ = self
                    .diagnostics_worker_msg_sender
                    .send(WorkerMessage::ShowDiagnostics(bufnr));
            }
            DiagnosticsAction::Cursor => {
                let bufnr = self.vim.bufnr("").await?;
                let _ = self
                    .diagnostics_worker_msg_sender
                    .send(WorkerMessage::ShowDiagnosticsUnderCursor(bufnr));
            }
            DiagnosticsAction::First => {
                self.navigate_diagnostics(All, First).await?;
            }
            DiagnosticsAction::Last => {
                self.navigate_diagnostics(All, Last).await?;
            }
            DiagnosticsAction::Next => {
                self.navigate_diagnostics(All, Next).await?;
            }
            DiagnosticsAction::Prev => {
                self.navigate_diagnostics(All, Prev).await?;
            }
            DiagnosticsAction::FirstError => {
                self.navigate_diagnostics(Error, First).await?;
            }
            DiagnosticsAction::LastError => {
                self.navigate_diagnostics(Error, Last).await?;
            }
            DiagnosticsAction::NextError => {
                self.navigate_diagnostics(Error, Next).await?;
            }
            DiagnosticsAction::PrevError => {
                self.navigate_diagnostics(Error, Prev).await?;
            }
            DiagnosticsAction::FirstWarn => {
                self.navigate_diagnostics(Warn, First).await?;
            }
            DiagnosticsAction::LastWarn => {
                self.navigate_diagnostics(Warn, Last).await?;
            }
            DiagnosticsAction::NextWarn => {
                self.navigate_diagnostics(Warn, Next).await?;
            }
            DiagnosticsAction::PrevWarn => {
                self.navigate_diagnostics(Warn, Prev).await?;
            }
            DiagnosticsAction::FirstHint => {
                self.navigate_diagnostics(Hint, First).await?;
            }
            DiagnosticsAction::LastHint => {
                self.navigate_diagnostics(Hint, Last).await?;
            }
            DiagnosticsAction::NextHint => {
                self.navigate_diagnostics(Hint, Next).await?;
            }
            DiagnosticsAction::PrevHint => {
                self.navigate_diagnostics(Hint, Prev).await?;
            }
        }

        Ok(())
    }
}
