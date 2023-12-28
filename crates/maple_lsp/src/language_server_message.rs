use crate::Error;
use rpc::{Failure, Id, Params, RpcMessage, RpcResponse, Success, Version};
use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// Requests from language server.
#[derive(Debug, PartialEq, Clone)]
pub enum LanguageServerRequest {
    WorkDoneProgressCreate(lsp::WorkDoneProgressCreateParams),
    ApplyWorkspaceEdit(lsp::ApplyWorkspaceEditParams),
    WorkspaceFolders,
    WorkspaceConfiguration(lsp::ConfigurationParams),
    RegisterCapability(lsp::RegistrationParams),
    UnregisterCapability(lsp::UnregistrationParams),
}

impl LanguageServerRequest {
    pub fn parse(method: &str, params: Params) -> Result<LanguageServerRequest, Error> {
        use lsp::request::Request;

        let request = match method {
            lsp::request::WorkDoneProgressCreate::METHOD => {
                let params: lsp::WorkDoneProgressCreateParams = params.parse()?;
                Self::WorkDoneProgressCreate(params)
            }
            lsp::request::ApplyWorkspaceEdit::METHOD => {
                let params: lsp::ApplyWorkspaceEditParams = params.parse()?;
                Self::ApplyWorkspaceEdit(params)
            }
            lsp::request::WorkspaceFoldersRequest::METHOD => Self::WorkspaceFolders,
            lsp::request::WorkspaceConfiguration::METHOD => {
                let params: lsp::ConfigurationParams = params.parse()?;
                Self::WorkspaceConfiguration(params)
            }
            lsp::request::RegisterCapability::METHOD => {
                let params: lsp::RegistrationParams = params.parse()?;
                Self::RegisterCapability(params)
            }
            lsp::request::UnregisterCapability::METHOD => {
                let params: lsp::UnregistrationParams = params.parse()?;
                Self::UnregisterCapability(params)
            }
            _ => {
                return Err(Error::Unhandled);
            }
        };
        Ok(request)
    }
}

/// Notifications from language server.
#[derive(Debug, PartialEq, Clone)]
pub enum LanguageServerNotification {
    // we inject this notification to signal the LSP is ready
    Initialized,
    // and this notification to signal that the LSP exited
    Exit,
    PublishDiagnostics(lsp::PublishDiagnosticsParams),
    ShowMessage(lsp::ShowMessageParams),
    LogMessage(lsp::LogMessageParams),
    ProgressMessage(lsp::ProgressParams),
}

impl LanguageServerNotification {
    pub fn parse(method: &str, params: Params) -> Result<LanguageServerNotification, Error> {
        use lsp::notification::Notification as _;

        let notification = match method {
            lsp::notification::Initialized::METHOD => Self::Initialized,
            lsp::notification::Exit::METHOD => Self::Exit,
            lsp::notification::PublishDiagnostics::METHOD => {
                let params: lsp::PublishDiagnosticsParams = params.parse()?;
                Self::PublishDiagnostics(params)
            }

            lsp::notification::ShowMessage::METHOD => {
                let params: lsp::ShowMessageParams = params.parse()?;
                Self::ShowMessage(params)
            }
            lsp::notification::LogMessage::METHOD => {
                let params: lsp::LogMessageParams = params.parse()?;
                Self::LogMessage(params)
            }
            lsp::notification::Progress::METHOD => {
                let params: lsp::ProgressParams = params.parse()?;
                Self::ProgressMessage(params)
            }
            _ => {
                // Unhandled notification
                return Err(Error::Unhandled);
            }
        };

        Ok(notification)
    }
}

#[derive(Debug)]
pub enum LanguageServerMessage {
    Request((Id, LanguageServerRequest)),
    Notification(LanguageServerNotification),
}

pub trait HandleLanguageServerMessage {
    fn handle_request(
        &mut self,
        id: Id,
        request: LanguageServerRequest,
    ) -> Result<Value, rpc::Error>;

    fn handle_notification(
        &mut self,
        notification: LanguageServerNotification,
    ) -> Result<(), Error>;
}

impl HandleLanguageServerMessage for () {
    fn handle_request(
        &mut self,
        _id: Id,
        _request: LanguageServerRequest,
    ) -> Result<Value, rpc::Error> {
        Ok(Value::Null)
    }

    fn handle_notification(
        &mut self,
        _notification: LanguageServerNotification,
    ) -> Result<(), Error> {
        Ok(())
    }
}

pub async fn handle_language_server_message<T: HandleLanguageServerMessage>(
    mut language_server_message_rx: UnboundedReceiver<LanguageServerMessage>,
    server_tx: UnboundedSender<RpcMessage>,
    mut language_server_message_handler: T,
) {
    // Reply a response to a language server RPC call.
    let reply_to_server = |id, result: Result<Value, rpc::Error>| {
        let output = match result {
            Ok(value) => RpcResponse::Success(Success {
                jsonrpc: Some(Version::V2),
                id,
                result: value,
            }),
            Err(error) => RpcResponse::Failure(Failure {
                jsonrpc: Some(Version::V2),
                id,
                error,
            }),
        };

        server_tx.send(RpcMessage::Response(output))?;

        Ok::<_, Error>(())
    };

    while let Some(lsp_server_msg) = language_server_message_rx.recv().await {
        match lsp_server_msg {
            LanguageServerMessage::Request((id, request)) => {
                let result = language_server_message_handler.handle_request(id.clone(), request);

                if let Err(err) = reply_to_server(id, result) {
                    tracing::error!("Failed to send response to server: {err:?}");
                    return;
                }
            }
            LanguageServerMessage::Notification(notification) => {
                if let Err(err) = language_server_message_handler.handle_notification(notification)
                {
                    tracing::error!(?err, "Failed to handle LanguageServerNotification");
                    return;
                }
            }
        }
    }
}
