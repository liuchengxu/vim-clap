use crate::stdio_server::Vim;
use maple_lsp::{
    lsp, HandleLanguageServerMessage, LanguageServerNotification, LanguageServerRequest,
};
use serde_json::Value;
use std::time::Instant;

#[derive(Debug)]
pub struct LanguageServerMessageHandler {
    name: String,
    last_lsp_update: Option<Instant>,
    vim: Vim,
}

impl LanguageServerMessageHandler {
    const LSP_UPDATE_DELAY: u128 = 50;

    pub fn new(name: String, vim: Vim) -> Self {
        Self {
            name,
            vim,
            last_lsp_update: None,
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
                .update_lsp_status(new.as_ref().unwrap_or(&self.name));
            self.last_lsp_update.replace(Instant::now());
        }
    }
}

impl HandleLanguageServerMessage for LanguageServerMessageHandler {
    fn handle_request(
        &mut self,
        id: rpc::Id,
        request: LanguageServerRequest,
    ) -> Result<Value, rpc::Error> {
        tracing::debug!(%id, "Processing language server request: {request:?}");

        match request {
            LanguageServerRequest::WorkDoneProgressCreate(_params) => {}
            _ => {}
        }

        Ok(Value::Null)
    }

    fn handle_notification(
        &mut self,
        notification: LanguageServerNotification,
    ) -> Result<(), maple_lsp::Error> {
        tracing::debug!("Processing language server notification: {notification:?}");

        match notification {
            LanguageServerNotification::ProgressMessage(params) => {
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
                            let _ = self.vim.update_lsp_status(&self.name);

                            // we want to render to clear any leftover spinners or messages
                            return Ok(());
                        }
                    }
                };

                let token_d: &dyn std::fmt::Display = match &token {
                    NumberOrString::Number(n) => n,
                    NumberOrString::String(s) => s,
                };

                let status = match parts {
                    (Some(title), Some(message), Some(percentage)) => {
                        format!("[{}] {}% {} - {}", token_d, percentage, title, message)
                    }
                    (Some(title), None, Some(percentage)) => {
                        format!("[{}] {}% {}", token_d, percentage, title)
                    }
                    (Some(title), Some(message), None) => {
                        format!("[{}] {} - {}", token_d, title, message)
                    }
                    (None, Some(message), Some(percentage)) => {
                        format!("[{}] {}% {}", token_d, percentage, message)
                    }
                    (Some(title), None, None) => {
                        format!("[{}] {}", token_d, title)
                    }
                    (None, Some(message), None) => {
                        format!("[{}] {}", token_d, message)
                    }
                    (None, None, Some(percentage)) => {
                        format!("[{}] {}%", token_d, percentage)
                    }
                    (None, None, None) => format!("[{}]", token_d),
                };

                if let WorkDoneProgress::End(_) = work {
                    let _ = self.vim.update_lsp_status(&self.name);
                } else {
                    self.update_lsp_status_gentlely(Some(status));
                }
            }
            _ => {}
        }

        Ok(())
    }
}
