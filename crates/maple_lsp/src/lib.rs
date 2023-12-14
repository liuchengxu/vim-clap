use futures_util::TryFutureExt;
use lsp::request::Request as RequestT;
use lsp::{
    GotoDefinitionParams, Position, ProgressToken, ServerCapabilities, TextDocumentIdentifier, Url,
};
use rpc::{
    Failure, Id, Params, RpcMessage, RpcNotification, RpcRequest, RpcResponse, Success, Version,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, OnceCell};

pub use lsp;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("client is not yet initialized")]
    Uninitialized,
    #[error("method {0} is unsupported by language server")]
    Unsupported(&'static str),
    #[error("client request returns a failure response: {0:?}")]
    RequestFailure(rpc::Failure),
    #[error("stream closed")]
    StreamClosed,
    #[error("Unhandled message")]
    Unhandled,
    #[error("failed to send response: {0:?}")]
    SendResponse(RpcResponse),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    JsonRpc(#[from] rpc::Error),
    #[error("failed to send raw message: {0}")]
    SendRawMessage(#[from] SendError<RpcMessage>),
    #[error("failed to send request: {0}")]
    SendRequest(#[from] SendError<(Id, oneshot::Sender<RpcResponse>)>),
    #[error("sender is dropped: {0}")]
    OneshotRecv(#[from] tokio::sync::oneshot::error::RecvError),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error("failed to parse server message: {0}")]
    BadServerMessage(String),
}

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

async fn handle_language_server_message<T: HandleLanguageServerMessage>(
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

fn recv_message_from_server<T: BufRead>(reader: &mut T) -> Result<String, Error> {
    let mut buffer = String::new();
    let mut content_length: Option<usize> = None;

    loop {
        buffer.clear();

        if reader.read_line(&mut buffer)? == 0 {
            return Err(Error::StreamClosed);
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
fn process_server_stdout(
    stdout: ChildStdout,
    mut response_sender_rx: UnboundedReceiver<(Id, oneshot::Sender<RpcResponse>)>,
    language_server_message_tx: UnboundedSender<LanguageServerMessage>,
) -> Result<(), Error> {
    let mut reader = Box::new(BufReader::new(stdout));

    let mut pending_requests = HashMap::new();

    loop {
        let line = recv_message_from_server(&mut reader)?;

        if line.is_empty() {
            continue;
        }

        match serde_json::from_str::<ServerMessage>(line.trim()) {
            Ok(rpc_message) => match rpc_message {
                ServerMessage::Response(output) => {
                    while let Ok((id, response_sender)) = response_sender_rx.try_recv() {
                        pending_requests.insert(id, response_sender);
                    }

                    if let Some(response_sender) = pending_requests.remove(output.id()) {
                        response_sender.send(output).map_err(Error::SendResponse)?;
                    }
                }
                ServerMessage::Call(Call::Request(request)) => {
                    let RpcRequest {
                        id, method, params, ..
                    } = request;
                    match LanguageServerRequest::parse(&method, params) {
                        Ok(request) => {
                            let _ = language_server_message_tx
                                .send(LanguageServerMessage::Request((id, request)));
                        }
                        Err(err) => {
                            tracing::error!(
                                ?err,
                                %method,
                                "Recv malformed server request",
                            );

                            return Err(rpc::Error {
                                code: rpc::ErrorCode::ParseError,
                                message: format!("Malformed server request: {method}"),
                                data: None,
                            }
                            .into());
                        }
                    };
                }
                ServerMessage::Call(Call::Notification(notification)) => {
                    let RpcNotification { method, params, .. } = notification;
                    match LanguageServerNotification::parse(&method, params) {
                        Ok(notification) => {
                            let _ = language_server_message_tx
                                .send(LanguageServerMessage::Notification(notification));
                        }
                        Err(err) => {
                            tracing::error!(
                                ?err,
                                %method,
                                "Recv malformed server notification",
                            );
                        }
                    }
                }
                ServerMessage::Call(Call::Invalid { id }) => {
                    tracing::error!("[handle_language_server_message] Invalid call: {id}");
                }
            },
            Err(err) => {
                return Err(Error::BadServerMessage(err.to_string()));
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

#[derive(Debug)]
pub struct Client {
    id: AtomicU64,
    capabilities: OnceCell<ServerCapabilities>,
    root_path: PathBuf,
    server_tx: UnboundedSender<RpcMessage>,
    response_sender_tx: UnboundedSender<(Id, oneshot::Sender<RpcResponse>)>,
    _server_process: Child,
}

fn process_server_stderr(stderr: ChildStderr) {
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
}

pub fn start_client<T: HandleLanguageServerMessage + Send + Sync + 'static>(
    server_executable: &str,
    args: &[String],
    workspace: &Path,
    language_server_message_handler: T,
    enable_snippets: bool,
) -> std::io::Result<Arc<Client>> {
    let client = Client::new(
        server_executable,
        args,
        workspace,
        language_server_message_handler,
    )?;

    tracing::debug!(server_executable, "A new LSP Client created");

    let client = Arc::new(client);

    // Initialize the client asynchronously.
    tokio::spawn({
        let client = client.clone();
        async move {
            let value = client
                .capabilities
                .get_or_try_init(|| {
                    client
                        .initialize(enable_snippets)
                        .map_ok(|response| response.capabilities)
                })
                .await;

            if let Err(e) = value {
                tracing::error!("failed to initialize language server: {}", e);
                return;
            }

            client
                .notify::<lsp::notification::Initialized>(lsp::InitializedParams {})
                .expect("Failed to notify Initialized");

            tracing::debug!("LSP client initialized");
        }
    });

    Ok(client)
}

impl Client {
    /// Constructs a new instance of LSP [`Client`].
    pub fn new<T: HandleLanguageServerMessage + Send + Sync + 'static>(
        server_executable: &str,
        args: &[String],
        workspace: &Path,
        language_server_message_handler: T,
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

        let (language_server_message_tx, language_server_message_rx) = unbounded_channel();

        std::thread::spawn({
            move || {
                if let Err(err) =
                    process_server_stdout(stdout, response_sender_rx, language_server_message_tx)
                {
                    tracing::error!(?err, "Failed to process server messages, exiting...");
                }
            }
        });

        tokio::spawn({
            let server_tx = payload_sender.clone();
            async move {
                handle_language_server_message(
                    language_server_message_rx,
                    server_tx,
                    language_server_message_handler,
                )
                .await;
            }
        });

        tokio::task::spawn_blocking(move || {
            process_server_stderr(stderr);
        });

        let client = Self {
            id: AtomicU64::new(0),
            server_tx: payload_sender,
            response_sender_tx,
            root_path: workspace.to_path_buf(),
            capabilities: OnceCell::new(),
            _server_process: process,
        };

        Ok(client)
    }

    pub fn is_initialized(&self) -> bool {
        self.capabilities.get().is_some()
    }

    pub fn capabilities(&self) -> &ServerCapabilities {
        self.capabilities
            .get()
            .expect("language server not yet initialized!")
    }

    pub fn include_text_on_save(&self) -> bool {
        let capabilities = self.capabilities();

        match &capabilities.text_document_sync {
            Some(lsp::TextDocumentSyncCapability::Options(lsp::TextDocumentSyncOptions {
                save:
                    Some(lsp::TextDocumentSyncSaveOptions::SaveOptions(lsp::SaveOptions {
                        include_text,
                    })),
                ..
            })) => include_text.unwrap_or(false),
            _ => false,
        }
    }

    pub async fn request<T: lsp::request::Request>(
        &self,
        params: T::Params,
    ) -> Result<T::Result, Error> {
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
            RpcResponse::Failure(err) => Err(Error::RequestFailure(err)),
        }
    }

    /// Send a RPC notification to the language server.
    pub fn notify<R: lsp::notification::Notification>(&self, params: R::Params) -> Result<(), Error>
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

    async fn initialize(&self, enable_snippets: bool) -> Result<lsp::InitializeResult, Error> {
        #[allow(deprecated)]
        let params = lsp::InitializeParams {
            process_id: Some(std::process::id()),
            workspace_folders: None,
            // root_path is obsolete, but some clients like pyright still use it so we specify both.
            // clients will prefer _uri if possible
            root_path: self.root_path.to_str().map(|s| s.to_string()),
            root_uri: Url::from_file_path(&self.root_path).ok(),
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
                name: String::from("vim-clap"),
                version: Some(String::from("v0.0.1")),
            }),
            locale: None, // TODO
        };

        self.request::<lsp::request::Initialize>(params).await
    }

    pub fn text_document_did_open(
        &self,
        uri: lsp::Url,
        version: i32,
        text: String,
        language_id: String,
    ) -> Result<(), Error> {
        self.notify::<lsp::notification::DidOpenTextDocument>(lsp::DidOpenTextDocumentParams {
            text_document: lsp::TextDocumentItem {
                uri,
                language_id,
                version,
                text,
            },
        })
    }

    pub fn text_document_did_close(
        &self,
        text_document: lsp::TextDocumentIdentifier,
    ) -> Result<(), Error> {
        self.notify::<lsp::notification::DidCloseTextDocument>(lsp::DidCloseTextDocumentParams {
            text_document,
        })
    }

    // will_save / will_save_wait_until

    pub fn text_document_did_save(
        &self,
        text_document: lsp::TextDocumentIdentifier,
        text: Option<String>,
    ) -> Result<(), Error> {
        self.notify::<lsp::notification::DidSaveTextDocument>(lsp::DidSaveTextDocumentParams {
            text_document,
            text,
        })
    }

    async fn goto_request<
        T: lsp::request::Request<
            Params = GotoDefinitionParams,
            Result = Option<lsp::GotoDefinitionResponse>,
        >,
    >(
        &self,
        text_document: lsp::TextDocumentIdentifier,
        position: lsp::Position,
        work_done_token: Option<lsp::ProgressToken>,
    ) -> Result<Vec<lsp::Location>, Error> {
        let params = lsp::GotoDefinitionParams {
            text_document_position_params: lsp::TextDocumentPositionParams {
                text_document,
                position,
            },
            work_done_progress_params: lsp::WorkDoneProgressParams { work_done_token },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        };

        let definitions = self.request::<T>(params).await?;

        Ok(to_locations(definitions))
    }

    pub async fn goto_definition(
        &self,
        text_document: lsp::TextDocumentIdentifier,
        position: lsp::Position,
        work_done_token: Option<lsp::ProgressToken>,
    ) -> Result<Vec<lsp::Location>, Error> {
        let capabilities = self.capabilities.get().ok_or(Error::Uninitialized)?;

        match capabilities.definition_provider {
            Some(lsp::OneOf::Left(true) | lsp::OneOf::Right(_)) => (),
            _ => return Err(Error::Unsupported(lsp::request::GotoDefinition::METHOD)),
        }

        self.goto_request::<lsp::request::GotoDefinition>(text_document, position, work_done_token)
            .await
    }

    pub async fn goto_declaration(
        &self,
        text_document: TextDocumentIdentifier,
        position: Position,
        work_done_token: Option<ProgressToken>,
    ) -> Result<Vec<lsp::Location>, Error> {
        let capabilities = self.capabilities.get().ok_or(Error::Uninitialized)?;

        match capabilities.declaration_provider {
            Some(
                lsp::DeclarationCapability::Simple(true)
                | lsp::DeclarationCapability::RegistrationOptions(_)
                | lsp::DeclarationCapability::Options(_),
            ) => (),
            _ => return Err(Error::Unsupported(lsp::request::GotoDeclaration::METHOD)),
        }

        self.goto_request::<lsp::request::GotoDeclaration>(text_document, position, work_done_token)
            .await
    }

    pub async fn goto_type_definition(
        &self,
        text_document: TextDocumentIdentifier,
        position: Position,
        work_done_token: Option<ProgressToken>,
    ) -> Result<Vec<lsp::Location>, Error> {
        let capabilities = self.capabilities.get().ok_or(Error::Uninitialized)?;

        match capabilities.type_definition_provider {
            Some(
                lsp::TypeDefinitionProviderCapability::Simple(true)
                | lsp::TypeDefinitionProviderCapability::Options(_),
            ) => (),
            _ => return Err(Error::Unsupported(lsp::request::GotoTypeDefinition::METHOD)),
        }

        self.goto_request::<lsp::request::GotoTypeDefinition>(
            text_document,
            position,
            work_done_token,
        )
        .await
    }

    pub async fn goto_implementation(
        &self,
        text_document: TextDocumentIdentifier,
        position: Position,
        work_done_token: Option<ProgressToken>,
    ) -> Result<Vec<lsp::Location>, Error> {
        let capabilities = self.capabilities.get().ok_or(Error::Uninitialized)?;

        match capabilities.type_definition_provider {
            Some(
                lsp::TypeDefinitionProviderCapability::Simple(true)
                | lsp::TypeDefinitionProviderCapability::Options(_),
            ) => (),
            _ => return Err(Error::Unsupported(lsp::request::GotoImplementation::METHOD)),
        }

        self.goto_request::<lsp::request::GotoImplementation>(
            text_document,
            position,
            work_done_token,
        )
        .await
    }

    pub async fn goto_reference(
        &self,
        text_document: TextDocumentIdentifier,
        position: Position,
        include_declaration: bool,
        work_done_token: Option<ProgressToken>,
    ) -> Result<Option<Vec<lsp::Location>>, Error> {
        let capabilities = self.capabilities.get().ok_or(Error::Uninitialized)?;

        match capabilities.references_provider {
            Some(lsp::OneOf::Left(true) | lsp::OneOf::Right(_)) => (),
            _ => return Err(Error::Unsupported(lsp::request::References::METHOD)),
        }

        let params = lsp::ReferenceParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document,
                position,
            },
            context: lsp::ReferenceContext {
                include_declaration,
            },
            work_done_progress_params: lsp::WorkDoneProgressParams { work_done_token },
            partial_result_params: lsp::PartialResultParams {
                partial_result_token: None,
            },
        };

        self.request::<lsp::request::References>(params).await
    }

    pub async fn document_symbols(
        &self,
        text_document: TextDocumentIdentifier,
    ) -> Result<Option<lsp::DocumentSymbolResponse>, Error> {
        let capabilities = self.capabilities.get().ok_or(Error::Uninitialized)?;

        match capabilities.document_symbol_provider {
            Some(lsp::OneOf::Left(true) | lsp::OneOf::Right(_)) => (),
            _ => {
                return Err(Error::Unsupported(
                    lsp::request::DocumentSymbolRequest::METHOD,
                ))
            }
        }

        let params = lsp::DocumentSymbolParams {
            text_document,
            work_done_progress_params: lsp::WorkDoneProgressParams::default(),
            partial_result_params: lsp::PartialResultParams::default(),
        };

        self.request::<lsp::request::DocumentSymbolRequest>(params)
            .await
    }

    // empty string to get all symbols
    pub async fn workspace_symbols(
        &self,
        query: String,
    ) -> Result<Option<lsp::WorkspaceSymbolResponse>, Error> {
        let capabilities = self.capabilities.get().ok_or(Error::Uninitialized)?;

        match capabilities.workspace_symbol_provider {
            Some(lsp::OneOf::Left(true) | lsp::OneOf::Right(_)) => (),
            _ => {
                return Err(Error::Unsupported(
                    lsp::request::WorkspaceSymbolRequest::METHOD,
                ))
            }
        }

        let params = lsp::WorkspaceSymbolParams {
            query,
            work_done_progress_params: lsp::WorkDoneProgressParams::default(),
            partial_result_params: lsp::PartialResultParams::default(),
        };

        self.request::<lsp::request::WorkspaceSymbolRequest>(params)
            .await
    }

    pub async fn code_actions(
        &self,
        text_document: lsp::TextDocumentIdentifier,
        range: lsp::Range,
        context: lsp::CodeActionContext,
    ) -> Result<Option<lsp::CodeActionResponse>, Error> {
        let capabilities = self.capabilities.get().ok_or(Error::Uninitialized)?;

        // Return early if the server does not support code actions.
        match capabilities.code_action_provider {
            Some(
                lsp::CodeActionProviderCapability::Simple(true)
                | lsp::CodeActionProviderCapability::Options(_),
            ) => (),
            _ => return Ok(None),
        }

        let params = lsp::CodeActionParams {
            text_document,
            range,
            context,
            work_done_progress_params: lsp::WorkDoneProgressParams::default(),
            partial_result_params: lsp::PartialResultParams::default(),
        };

        self.request::<lsp::request::CodeActionRequest>(params)
            .await
    }

    pub async fn shutdown(&self) -> Result<(), Error> {
        self.request::<lsp::request::Shutdown>(()).await
    }

    pub async fn exit(&self) -> Result<(), Error> {
        self.notify::<lsp::notification::Exit>(())
    }
}

fn to_locations(definitions: Option<lsp::GotoDefinitionResponse>) -> Vec<lsp::Location> {
    match definitions {
        Some(lsp::GotoDefinitionResponse::Scalar(location)) => vec![location],
        Some(lsp::GotoDefinitionResponse::Array(locations)) => locations,
        Some(lsp::GotoDefinitionResponse::Link(locations)) => locations
            .into_iter()
            .map(|location_link| lsp::Location {
                uri: location_link.target_uri,
                range: location_link.target_range,
            })
            .collect(),
        None => Vec::new(),
    }
}
