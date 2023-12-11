use crate::stdio_server::Vim;
use maple_lsp::{
    lsp, HandleLanguageServerMessage, LanguageServerNotification, LanguageServerRequest,
};
use serde_json::Value;

#[derive(Debug)]
pub struct LanguageServerMessageHandler {
    name: String,
    vim: Vim,
}

impl LanguageServerMessageHandler {
    pub fn new(name: String, vim: Vim) -> Self {
        Self { name, vim }
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
            LanguageServerRequest::WorkDoneProgressCreate(params) => {}
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
                            // self.lsp_progress.end_progress(server_id, &token);
                            // if !self.lsp_progress.is_progressing(server_id) {
                            // editor_view.spinners_mut().get_or_create(server_id).stop();
                            // }
                            // self.editor.clear_status();

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
                    // Update LSP progress.
                    let _ = self.vim.update_lsp_status(status);
                }
            }
            _ => {}
        }

        Ok(())
    }
}
