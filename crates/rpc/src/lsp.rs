use lsp::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;

use crate::types::{
    Failure, Id, Params, RpcMessage, RpcNotification, RpcRequest, RpcResponse, Success, Version,
};
use crate::{Error, RpcError};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
pub enum Call {
    Request(RpcRequest),
    Notification(RpcNotification),
    Invalid {
        // We can attempt to salvage the id out of the invalid request
        // for better debugging
        #[serde(default = "default_id")]
        id: Id,
    },
}

fn default_id() -> Id {
    Id::Null
}

impl From<RpcRequest> for Call {
    fn from(request: RpcRequest) -> Self {
        Call::Request(request)
    }
}

impl From<RpcNotification> for Call {
    fn from(notification: RpcNotification) -> Self {
        Call::Notification(notification)
    }
}

/// A type representing all possible values sent from the server to the client.
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(untagged)]
enum ServerMessage {
    /// A regular JSON-RPC request output (single response).
    Response(RpcResponse),
    /// A JSON-RPC request or notification.
    Call(Call),
}

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
    pub fn parse(method: &str, params: Params) -> Result<LanguageServerRequest, RpcError> {
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
                return Err(RpcError::Unhandled);
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
    pub fn parse(method: &str, params: Params) -> Result<LanguageServerNotification, RpcError> {
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
                return Err(RpcError::Unhandled);
            }
        };

        Ok(notification)
    }
}

enum LanguageServerMessage {
    Request((Id, LanguageServerRequest)),
    Notification(LanguageServerNotification),
}

async fn handle_language_server_message(
    mut server_message_rx: UnboundedReceiver<LanguageServerMessage>,
    server_tx: UnboundedSender<RpcMessage>,
) {
    // Reply a response to a language server RPC call.
    let reply_to_server = |id, result| {
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

        Ok::<_, RpcError>(())
    };

    while let Some(lsp_server_msg) = server_message_rx.recv().await {
        match lsp_server_msg {
            LanguageServerMessage::Request((id, request)) => {
                let result = {
                    println!("========== [handle_language_server_message] TODO handle LSP request: {request:?}");
                    Ok(Value::Null)
                };

                if let Err(err) = reply_to_server(id, result) {
                    tracing::error!("Failed to send response to server: {err:?}");
                    return;
                }
            }
            LanguageServerMessage::Notification(notification) => {
                println!("========== [handle_language_server_message] TODO handle LSP notification: {notification:?}");
            }
        }
    }
}

enum LspHeader {
    ContentType,
    ContentLength(usize),
}

fn parse_header(s: &str) -> std::io::Result<LspHeader> {
    const CONTENT_LENGTH: &str = "content-length";
    const CONTENT_TYPE: &str = "content-type";

    let split = s
        .splitn(2, ": ")
        .map(|s| s.trim().to_lowercase())
        .collect::<Vec<String>>();

    if split.len() != 2 {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Malformed"));
    };

    match split[0].as_ref() {
        CONTENT_TYPE => Ok(LspHeader::ContentType),
        CONTENT_LENGTH => Ok(LspHeader::ContentLength(split[1].parse::<usize>().unwrap())),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Unknown LSP header line",
        )),
    }
}

fn recv_message_from_server<T: BufRead>(reader: &mut T) -> Result<String, RpcError> {
    let mut buffer = String::new();
    let mut content_length: Option<usize> = None;

    loop {
        buffer.clear();

        if reader.read_line(&mut buffer)? == 0 {
            return Err(RpcError::StreamClosed);
        }

        match &buffer {
            s if s.trim().is_empty() => break,
            s => {
                match parse_header(s)? {
                    LspHeader::ContentLength(len) => {
                        content_length.replace(len);
                    }
                    LspHeader::ContentType => {}
                };
            }
        };
    }

    let content_length = content_length.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("missing content-length header: {buffer}"),
        )
    })?;

    let mut body_buffer = vec![0; content_length];
    reader.read_exact(&mut body_buffer)?;

    let body = String::from_utf8(body_buffer).expect("LSP server must use utf8");

    Ok(body)
}

/// Process the messages from language server.
fn process_server_messages<T: BufRead>(
    mut reader: T,
    mut response_sender_rx: UnboundedReceiver<(Id, oneshot::Sender<RpcResponse>)>,
    server_message_tx: UnboundedSender<LanguageServerMessage>,
) -> Result<(), RpcError> {
    let mut pending_response_senders = HashMap::new();

    loop {
        let line = recv_message_from_server(&mut reader)?;

        match serde_json::from_str::<ServerMessage>(line.trim()) {
            Ok(rpc_message) => match rpc_message {
                ServerMessage::Response(output) => {
                    while let Ok((id, response_sender)) = response_sender_rx.try_recv() {
                        pending_response_senders.insert(id, response_sender);
                    }

                    if let Some(response_sender) = pending_response_senders.remove(output.id()) {
                        response_sender.send(output).map_err(|response| {
                            tracing::debug!(
                                "Failed to send response from language server: {response:?}"
                            );
                            RpcError::SendResponse(response)
                        })?;
                    }
                }
                ServerMessage::Call(Call::Request(request)) => {
                    let id = request.id.clone();
                    match LanguageServerRequest::parse(&request.method, request.params) {
                        Ok(request) => {
                            let _ = server_message_tx
                                .send(LanguageServerMessage::Request((id, request)));
                        }
                        Err(err) => {
                            tracing::error!(
                                ?err,
                                "Language Server: Received malformed LSP request: {}",
                                request.method
                            );

                            return Err(Error {
                                code: crate::ErrorCode::ParseError,
                                message: format!("Malformed server request: {}", request.method),
                                data: None,
                            }
                            .into());
                        }
                    };
                }
                ServerMessage::Call(Call::Notification(notification)) => {
                    match LanguageServerNotification::parse(
                        &notification.method,
                        notification.params,
                    ) {
                        Ok(notification) => {
                            let _ = server_message_tx
                                .send(LanguageServerMessage::Notification(notification));
                        }
                        Err(err) => {
                            tracing::error!(
                                ?err,
                                "Language Server: Received malformed LSP notification: {}",
                                notification.method
                            );
                        }
                    }
                }
                ServerMessage::Call(Call::Invalid { id }) => {
                    tracing::error!("[handle_language_server_message] Invalid call: {id}");
                }
            },
            Err(err) => {
                return Err(RpcError::DeserializeFailure(format!(
                    "Failed to deserialize ServerMessage: {err:?}"
                )));
            }
        }
    }
}

fn value_to_params(value: Value) -> Params {
    match value {
        Value::Null => Params::None,
        Value::Array(vec) => Params::Array(vec),
        Value::Bool(_) | Value::Number(_) | Value::String(_) => Params::Array(vec![value]),
        Value::Object(map) => Params::Map(map),
    }
}

fn start_server_program(cmd: &str, args: &[String], workspace: &Path) -> std::io::Result<Child> {
    let mut process = Command::new(cmd);

    process.current_dir(workspace);
    process.args(args);

    let child = process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Make sure the process is reaped on drop.
        // .kill_on_drop(true)
        .spawn()?;

    Ok(child)
}

pub struct Client {
    id: AtomicU64,
    server_tx: UnboundedSender<RpcMessage>,
    response_sender_tx: UnboundedSender<(Id, oneshot::Sender<RpcResponse>)>,
}

impl Client {
    /// Constructs a new instance of LSP [`Client`].
    pub fn new(
        server_executable: &str,
        args: &[String],
        workspace: &Path,
    ) -> std::io::Result<Self> {
        let mut process = start_server_program(server_executable, args, workspace)?;

        let stdin = process.stdin.take().expect("Failed to open stdin");
        let stdout = process.stdout.take().expect("Failed to open stdout");
        let stderr = process.stderr.take().expect("Failed to open stderr");

        let (payload_sender, mut payload_receiver) = unbounded_channel();

        // Send requests to language server.
        tokio::spawn(async move {
            let mut writer = Box::new(BufWriter::new(stdin));

            while let Some(msg) = payload_receiver.recv().await {
                if let Ok(msg) = serde_json::to_string(&msg) {
                    let msg = format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg);
                    let _ = writer.write(msg.as_bytes());
                    let _ = writer.flush();
                }
            }
        });

        let (response_sender_tx, response_sender_rx): (
            UnboundedSender<(Id, oneshot::Sender<RpcResponse>)>,
            _,
        ) = unbounded_channel();

        let (server_message_tx, server_message_rx) = unbounded_channel();

        std::thread::spawn({
            let reader = Box::new(BufReader::new(stdout));

            move || {
                if let Err(err) =
                    process_server_messages(reader, response_sender_rx, server_message_tx)
                {
                    tracing::error!(?err, "Failed to process server messages, exiting...");
                }
            }
        });

        tokio::spawn({
            let server_tx = payload_sender.clone();
            async move {
                handle_language_server_message(server_message_rx, server_tx).await;
            }
        });

        tokio::task::spawn_blocking(move || {
            let mut reader = Box::new(BufReader::new(stderr));

            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(n) => {
                        if n == 0 {
                            // Stream closed.
                            return;
                        }
                        tracing::error!("lsp server error: {}", line.trim_end());
                    }
                    Err(err) => {
                        tracing::error!("Error occurred at reading child stderr: {err:?}");
                        return;
                    }
                }
            }
        });

        let client = Self {
            id: AtomicU64::new(0),
            server_tx: payload_sender,
            response_sender_tx,
        };

        Ok(client)
    }

    pub async fn request<T: lsp::request::Request>(
        &self,
        params: T::Params,
    ) -> Result<T::Result, RpcError> {
        let params = serde_json::to_value(params)?;

        let id = self.id.fetch_add(1, Ordering::SeqCst);

        let rpc_request = RpcRequest {
            jsonrpc: Some(Version::V2),
            id: Id::Num(id),
            method: T::METHOD.to_string(),
            params: value_to_params(params),
        };

        let (request_result_tx, request_result_rx) = tokio::sync::oneshot::channel();
        // Request result will be sent back in a RpcResponse message.
        self.response_sender_tx
            .send((Id::Num(id), request_result_tx))?;
        self.server_tx.send(RpcMessage::Request(rpc_request))?;
        match request_result_rx.await? {
            RpcResponse::Success(ok) => Ok(serde_json::from_value(ok.result)?),
            RpcResponse::Failure(err) => Err(RpcError::Request(format!(
                "RpcClient request error: {err:?}"
            ))),
        }
    }

    /// Send a RPC notification to the language server.
    pub fn notify<R: lsp::notification::Notification>(
        &self,
        params: R::Params,
    ) -> Result<(), RpcError>
    where
        R::Params: serde::Serialize,
    {
        let params = serde_json::to_value(params)?;

        let notification = RpcNotification {
            jsonrpc: Some(Version::V2),
            method: R::METHOD.to_string(),
            params: value_to_params(params),
        };

        self.server_tx
            .send(RpcMessage::Notification(notification))?;

        Ok(())
    }

    pub async fn initialize(
        &self,
        root_path: Option<String>,
        root_uri: Option<Url>,
        enable_snippets: bool,
    ) -> Result<lsp::InitializeResult, RpcError> {
        #[allow(deprecated)]
        let params = lsp::InitializeParams {
            process_id: Some(std::process::id()),
            workspace_folders: None,
            // root_path is obsolete, but some clients like pyright still use it so we specify both.
            // clients will prefer _uri if possible
            root_path: root_path.clone(),
            root_uri: root_uri.clone(),
            initialization_options: None,
            capabilities: lsp::ClientCapabilities {
                workspace: Some(lsp::WorkspaceClientCapabilities {
                    configuration: Some(true),
                    did_change_configuration: Some(lsp::DynamicRegistrationClientCapabilities {
                        dynamic_registration: Some(false),
                    }),
                    workspace_folders: Some(true),
                    apply_edit: Some(true),
                    symbol: Some(lsp::WorkspaceSymbolClientCapabilities {
                        dynamic_registration: Some(false),
                        ..Default::default()
                    }),
                    execute_command: Some(lsp::DynamicRegistrationClientCapabilities {
                        dynamic_registration: Some(false),
                    }),
                    inlay_hint: Some(lsp::InlayHintWorkspaceClientCapabilities {
                        refresh_support: Some(false),
                    }),
                    workspace_edit: Some(lsp::WorkspaceEditClientCapabilities {
                        document_changes: Some(true),
                        resource_operations: Some(vec![
                            lsp::ResourceOperationKind::Create,
                            lsp::ResourceOperationKind::Rename,
                            lsp::ResourceOperationKind::Delete,
                        ]),
                        failure_handling: Some(lsp::FailureHandlingKind::Abort),
                        normalizes_line_endings: Some(false),
                        change_annotation_support: None,
                    }),
                    did_change_watched_files: Some(lsp::DidChangeWatchedFilesClientCapabilities {
                        dynamic_registration: Some(true),
                        relative_pattern_support: Some(false),
                    }),
                    file_operations: Some(lsp::WorkspaceFileOperationsClientCapabilities {
                        will_rename: Some(true),
                        did_rename: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                text_document: Some(lsp::TextDocumentClientCapabilities {
                    completion: Some(lsp::CompletionClientCapabilities {
                        completion_item: Some(lsp::CompletionItemCapability {
                            snippet_support: Some(enable_snippets),
                            resolve_support: Some(lsp::CompletionItemCapabilityResolveSupport {
                                properties: vec![
                                    String::from("documentation"),
                                    String::from("detail"),
                                    String::from("additionalTextEdits"),
                                ],
                            }),
                            insert_replace_support: Some(true),
                            deprecated_support: Some(true),
                            tag_support: Some(lsp::TagSupport {
                                value_set: vec![lsp::CompletionItemTag::DEPRECATED],
                            }),
                            ..Default::default()
                        }),
                        completion_item_kind: Some(lsp::CompletionItemKindCapability {
                            ..Default::default()
                        }),
                        context_support: None, // additional context information Some(true)
                        ..Default::default()
                    }),
                    hover: Some(lsp::HoverClientCapabilities {
                        // if not specified, rust-analyzer returns plaintext marked as markdown but
                        // badly formatted.
                        content_format: Some(vec![lsp::MarkupKind::Markdown]),
                        ..Default::default()
                    }),
                    signature_help: Some(lsp::SignatureHelpClientCapabilities {
                        signature_information: Some(lsp::SignatureInformationSettings {
                            documentation_format: Some(vec![lsp::MarkupKind::Markdown]),
                            parameter_information: Some(lsp::ParameterInformationSettings {
                                label_offset_support: Some(true),
                            }),
                            active_parameter_support: Some(true),
                        }),
                        ..Default::default()
                    }),
                    rename: Some(lsp::RenameClientCapabilities {
                        dynamic_registration: Some(false),
                        prepare_support: Some(true),
                        prepare_support_default_behavior: None,
                        honors_change_annotations: Some(false),
                    }),
                    code_action: Some(lsp::CodeActionClientCapabilities {
                        code_action_literal_support: Some(lsp::CodeActionLiteralSupport {
                            code_action_kind: lsp::CodeActionKindLiteralSupport {
                                value_set: [
                                    lsp::CodeActionKind::EMPTY,
                                    lsp::CodeActionKind::QUICKFIX,
                                    lsp::CodeActionKind::REFACTOR,
                                    lsp::CodeActionKind::REFACTOR_EXTRACT,
                                    lsp::CodeActionKind::REFACTOR_INLINE,
                                    lsp::CodeActionKind::REFACTOR_REWRITE,
                                    lsp::CodeActionKind::SOURCE,
                                    lsp::CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
                                ]
                                .iter()
                                .map(|kind| kind.as_str().to_string())
                                .collect(),
                            },
                        }),
                        is_preferred_support: Some(true),
                        disabled_support: Some(true),
                        data_support: Some(true),
                        resolve_support: Some(lsp::CodeActionCapabilityResolveSupport {
                            properties: vec!["edit".to_owned(), "command".to_owned()],
                        }),
                        ..Default::default()
                    }),
                    publish_diagnostics: Some(lsp::PublishDiagnosticsClientCapabilities {
                        version_support: Some(true),
                        ..Default::default()
                    }),
                    inlay_hint: Some(lsp::InlayHintClientCapabilities {
                        dynamic_registration: Some(false),
                        resolve_support: None,
                    }),
                    ..Default::default()
                }),
                window: Some(lsp::WindowClientCapabilities {
                    work_done_progress: Some(true),
                    ..Default::default()
                }),
                general: Some(lsp::GeneralClientCapabilities {
                    position_encodings: Some(vec![
                        lsp::PositionEncodingKind::UTF8,
                        lsp::PositionEncodingKind::UTF32,
                        lsp::PositionEncodingKind::UTF16,
                    ]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            trace: None,
            client_info: Some(lsp::ClientInfo {
                name: String::from("xlc"),
                version: Some(String::from("v0.0.1")),
            }),
            locale: None, // TODO
        };

        self.request::<lsp::request::Initialize>(params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lsp_works() {
        let line = r#"{"jsonrpc":"2.0","id":0,"method":"window/workDoneProgress/create","params":{"token":"rustAnalyzer/Fetching"}}"#;
        let msg: ServerMessage = serde_json::from_str(line).unwrap();
        println!("msg: {msg:?}");

        let root_path = "/home/xlc/.vim/plugged/vim-clap/crates/rpc";
        let client = Client::new(
            "/home/xlc/.cargo/bin/rust-analyzer",
            &[],
            Path::new(root_path),
        )
        .unwrap();

        let res = client
            .initialize(Some(root_path.to_string()), None, false)
            .await;

        if res.is_ok() {
            client
                .notify::<lsp::notification::Initialized>(lsp::InitializedParams {})
                .unwrap();
        }

        println!("========== res: {res:?}");

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}