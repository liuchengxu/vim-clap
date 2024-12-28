use crate::stdio_server::diagnostics_worker::WorkerMessage as DiagnosticsWorkerMessage;
use crate::stdio_server::Vim;
use maple_lsp::{
    lsp, HandleLanguageServerMessage, LanguageServerNotification, LanguageServerRequest,
};
use serde_json::Value;
use std::time::Instant;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub struct LanguageServerMessageHandler {
    server_name: String,
    last_lsp_update: Option<Instant>,
    diagnostics_worker_msg_sender: UnboundedSender<DiagnosticsWorkerMessage>,
    vim: Vim,
}

impl LanguageServerMessageHandler {
    const LSP_UPDATE_DELAY: u128 = 50;

    pub fn new(
        server_name: String,
        vim: Vim,
        diagnostics_worker_msg_sender: UnboundedSender<DiagnosticsWorkerMessage>,
    ) -> Self {
        Self {
            server_name,
            vim,
            last_lsp_update: None,
            diagnostics_worker_msg_sender,
        }
    }

    /// Update the lsp status if a certain time delay has passed since the last update.
    fn update_lsp_status_gentlely(&mut self, new: Option<String>) {
        let should_update = match self.last_lsp_update {
            Some(last_update) => last_update.elapsed().as_millis() > Self::LSP_UPDATE_DELAY,
            None => true,
        };

        if should_update {
            let _ = self
                .vim
                .update_lsp_status(new.as_ref().unwrap_or(&self.server_name));
            self.last_lsp_update.replace(Instant::now());
        }
    }

    fn handle_message(&self, show_message: bool, msg_type: lsp::MessageType, message: String) {
        let msg_type = match msg_type {
            lsp::MessageType::ERROR => "ERROR",
            lsp::MessageType::WARNING => "WARN",
            lsp::MessageType::INFO => "INFO",
            lsp::MessageType::LOG => "LOG",
            _ => return,
        };
        if show_message {
            let _ = self
                .vim
                .echo_message(format!("[{}] [{msg_type}] {message}", self.server_name));
        } else {
            tracing::debug!("[{}] [{msg_type}] {message}", self.server_name);
        }
    }

    fn handle_progress_message(
        &mut self,
        params: lsp::ProgressParams,
    ) -> Result<(), maple_lsp::Error> {
        use lsp::{
            NumberOrString, ProgressParams, ProgressParamsValue, WorkDoneProgress,
            WorkDoneProgressBegin, WorkDoneProgressEnd, WorkDoneProgressReport,
        };

        let ProgressParams { token, value } = params;

        let ProgressParamsValue::WorkDone(work) = value;

        let parts = match &work {
            WorkDoneProgress::Begin(WorkDoneProgressBegin {
                title,
                message,
                percentage,
                ..
            }) => (Some(title), message, percentage),
            WorkDoneProgress::Report(WorkDoneProgressReport {
                message,
                percentage,
                ..
            }) => (None, message, percentage),
            WorkDoneProgress::End(WorkDoneProgressEnd { message }) => {
                if message.is_some() {
                    (None, message, &None)
                } else {
                    // End progress.
                    let _ = self.vim.update_lsp_status(&self.server_name);

                    // we want to render to clear any leftover spinners or messages
                    return Ok(());
                }
            }
        };

        if let WorkDoneProgress::End(_) = work {
            let _ = self.vim.update_lsp_status(&self.server_name);
        } else {
            let token_d: &dyn std::fmt::Display = match &token {
                NumberOrString::Number(n) => n,
                NumberOrString::String(s) => s,
            };

            let status = match parts {
                (Some(title), Some(message), Some(percentage)) => {
                    format!("[{token_d}] {percentage}% {title} - {message}")
                }
                (Some(title), Some(message), None) => {
                    format!("[{token_d}] {title} - {message}")
                }
                (Some(title), None, Some(percentage)) => {
                    format!("[{token_d}] {percentage}% {title}")
                }
                (Some(title), None, None) => {
                    format!("[{token_d}] {title}")
                }
                (None, Some(message), Some(percentage)) => {
                    format!("[{token_d}] {percentage}% {message}")
                }
                (None, Some(message), None) => {
                    format!("[{token_d}] {message}")
                }
                (None, None, Some(percentage)) => {
                    format!("[{token_d}] {percentage}%")
                }
                (None, None, None) => format!("[{token_d}]"),
            };
            self.update_lsp_status_gentlely(Some(status));
        }

        Ok(())
    }
}

impl HandleLanguageServerMessage for LanguageServerMessageHandler {
    fn handle_request(
        &mut self,
        id: rpc::Id,
        request: LanguageServerRequest,
    ) -> Result<Value, rpc::Error> {
        tracing::trace!(%id, "Processing language server request: {request:?}");

        // match request {
        // LanguageServerRequest::WorkDoneProgressCreate(_params) => {}
        // _ => {}
        // }

        Ok(Value::Null)
    }

    fn handle_notification(
        &mut self,
        notification: LanguageServerNotification,
    ) -> Result<(), maple_lsp::Error> {
        tracing::trace!("Processing language server notification: {notification:?}");

        match notification {
            LanguageServerNotification::ProgressMessage(params) => {
                self.handle_progress_message(params)?;
            }
            LanguageServerNotification::PublishDiagnostics(params) => {
                if !params.diagnostics.is_empty() {
                    // Notify the diagnostics worker.
                    if self
                        .diagnostics_worker_msg_sender
                        .send(DiagnosticsWorkerMessage::LspDiagnostics(params))
                        .is_err()
                    {
                        tracing::error!("Failed to send diagnostics from LSP");
                    }
                }
            }
            LanguageServerNotification::ShowMessage(lsp::ShowMessageParams { typ, message }) => {
                self.handle_message(true, typ, message);
            }
            LanguageServerNotification::LogMessage(lsp::LogMessageParams { typ, message }) => {
                self.handle_message(false, typ, message);
            }
            _ => {
                tracing::debug!("TODO: handle language server notification: {notification:?}");
            }
        }

        Ok(())
    }
}
