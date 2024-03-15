mod json_patch;
mod language_server_message;

use futures_util::TryFutureExt;
use lsp::request::Request as RequestT;
use lsp::{
    GotoDefinitionParams, OneOf, Position, ProgressToken, ServerCapabilities,
    TextDocumentIdentifier, Url,
};
use parking_lot::Mutex;
use rpc::{Id, Params, RpcMessage, RpcNotification, RpcRequest, RpcResponse, Version};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, OnceCell};

pub use self::language_server_message::{
    HandleLanguageServerMessage, LanguageServerMessage, LanguageServerNotification,
    LanguageServerRequest,
};
pub use lsp;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("client is not yet initialized")]
    Uninitialized,
    #[error("failed to initialize language server")]
    FailedToInitServer,
    #[error("method {0} is unsupported by language server")]
    Unsupported(&'static str),
    #[error("received a failure response: {0:?}")]
    ResponseFailure(rpc::Failure),
    #[error("stream closed")]
    StreamClosed,
    #[error("Unhandled message")]
    Unhandled,
    #[error("language server executable not found: {0}")]
    ServerExecutableNotFound(String),
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

/// Find an LSP workspace of a file using the following mechanism:
/// * if the file is outside `workspace` return `None`.
/// * start at `file` and search the file tree upward, stop the search
///   at the first `root_dirs` entry that contains `file`.
/// * if no `root_dirs` matches `file` stop at workspace,
///   - returns the top most directory that contains a `root_marker`.
/// * If no root marker and we stopped at a `root_dirs` entry,
///   - return the directory we stopped at.
/// * If we stopped at `workspace` instead:
///   - `workspace_is_cwd == false` return `None`
///   - `workspace_is_cwd == true` return `workspace`
pub fn find_lsp_workspace(
    file: &str,
    root_markers: &[String],
    root_dirs: &[PathBuf],
    workspace: &Path,
    workspace_is_cwd: bool,
) -> Option<PathBuf> {
    let file = std::path::Path::new(file);
    let mut file = if file.is_absolute() {
        file.to_path_buf()
    } else {
        let current_dir = paths::current_working_dir();
        current_dir.join(file)
    };
    file = paths::get_normalized_path(&file);

    if !file.starts_with(workspace) {
        return None;
    }

    let mut top_marker = None;
    for ancestor in file.ancestors() {
        if root_markers
            .iter()
            .any(|marker| ancestor.join(marker).exists())
        {
            top_marker = Some(ancestor);
        }

        if root_dirs
            .iter()
            .any(|root_dir| paths::get_normalized_path(&workspace.join(root_dir)) == ancestor)
        {
            // if the worskapce is the cwd do not search any higher for workspaces
            // but specify
            return Some(top_marker.unwrap_or(workspace).to_owned());
        }
        if ancestor == workspace {
            // if the workspace is the CWD, let the LSP decide what the workspace
            // is
            return top_marker
                .or_else(|| (!workspace_is_cwd).then_some(workspace))
                .map(Path::to_owned);
        }
    }

    debug_assert!(false, "workspace must be an ancestor of <file>");

    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LanguageServerConfig {
    /// Language server executable, e.g., `rust-analyzer`.
    pub command: String,

    /// Arguments passed to the language server executable.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,

    /// Represents the optional `initialization_options`.
    #[serde(default, skip_serializing, deserialize_with = "deserialize_lsp_config")]
    pub config: Option<serde_json::Value>,
}

impl LanguageServerConfig {
    pub fn server_name(&self) -> String {
        self.command
            .rsplit_once(std::path::MAIN_SEPARATOR)
            .map(|(_, binary)| binary)
            .unwrap_or(&self.command)
            .to_owned()
    }

    pub fn update_config(&mut self, user_config: serde_json::Value) {
        if let Some(c) = self.config.as_mut() {
            json_patch::merge(c, user_config);
        }
    }
}

fn deserialize_lsp_config<'de, D>(deserializer: D) -> Result<Option<serde_json::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<toml::Value>::deserialize(deserializer)?
        .map(|toml| toml.try_into().map_err(serde::de::Error::custom))
        .transpose()
}

#[derive(Debug, Clone)]
pub struct ClientParams {
    pub language_server_config: LanguageServerConfig,
    pub manual_roots: Vec<PathBuf>,
    pub enable_snippets: bool,
}

pub async fn start_client<T>(
    client_params: ClientParams,
    name: String,
    doc_path: Option<PathBuf>,
    root_markers: Vec<String>,
    language_server_message_handler: T,
) -> Result<Arc<Client>, Error>
where
    T: HandleLanguageServerMessage + Send + Sync + 'static,
{
    let ClientParams {
        language_server_config,
        manual_roots,
        enable_snippets,
    } = client_params;

    let LanguageServerConfig {
        command,
        args,
        config: initialization_options,
    } = language_server_config;

    let client = Client::new(
        &command,
        &args,
        name,
        &root_markers,
        &manual_roots,
        doc_path,
        language_server_message_handler,
    )?;

    tracing::debug!(?command, "A new LSP Client created: {client:?}");

    let client = Arc::new(client);

    let value = client
        .capabilities
        .get_or_try_init(|| {
            client
                .initialize(enable_snippets, initialization_options)
                .map_ok(|response| response.capabilities)
        })
        .await;

    if let Err(e) = value {
        tracing::error!("failed to initialize language server: {e:?}");
        return Err(Error::FailedToInitServer);
    }

    client.notify::<lsp::notification::Initialized>(lsp::InitializedParams {})?;

    tracing::debug!("LSP client initialized");

    Ok(client)
}

/// Finds the current workspace folder.
/// Used as a ceiling dir for LSP root resolution, the filepicker and potentially as a future filewatching root
///
/// This function starts searching the FS upward from the CWD
/// and returns the first directory that contains either `.git` or `.helix`.
/// If no workspace was found returns (CWD, true).
/// Otherwise (workspace, false) is returned
pub fn find_workspace() -> (PathBuf, bool) {
    let current_dir = paths::current_working_dir();
    for ancestor in current_dir.ancestors() {
        if ancestor.join(".git").exists() {
            return (ancestor.to_owned(), false);
        }
    }

    (current_dir.clone(), true)
}

pub fn workspace_for_uri(uri: lsp::Url) -> lsp::WorkspaceFolder {
    lsp::WorkspaceFolder {
        name: uri
            .path_segments()
            .and_then(|segments| segments.last())
            .map(|basename| basename.to_string())
            .unwrap_or_default(),
        uri,
    }
}

fn spawn_task_stdin(stdin: ChildStdin) -> UnboundedSender<RpcMessage> {
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

    payload_sender
}

#[derive(Debug)]
pub struct Client {
    id: AtomicU64,
    _name: String,
    root_path: PathBuf,
    root_uri: Option<Url>,
    workspace_folders: Mutex<Vec<lsp::WorkspaceFolder>>,
    capabilities: OnceCell<ServerCapabilities>,
    server_tx: UnboundedSender<RpcMessage>,
    response_sender_tx: UnboundedSender<(Id, oneshot::Sender<RpcResponse>)>,
    _server_process: Child,
}

impl Client {
    /// Constructs a new instance of LSP [`Client`].
    pub fn new<T: HandleLanguageServerMessage + Send + Sync + 'static>(
        cmd: &str,
        args: &[String],
        name: String,
        root_markers: &[String],
        manual_roots: &[PathBuf],
        doc_path: Option<PathBuf>,
        language_server_message_handler: T,
    ) -> Result<Self, Error> {
        let cmd =
            which::which(cmd).map_err(|_| Error::ServerExecutableNotFound(cmd.to_string()))?;

        // Redir the server stderr info to a log file, which will be truncated if
        // it exists beforehand.
        let server_log_dir = dirs::Dirs::base()
            .data_dir()
            .join("vimclap")
            .join("logs")
            .join("language_server_stderr");

        if !server_log_dir.exists() {
            std::fs::create_dir_all(&server_log_dir).ok();
        }

        let log_path = server_log_dir.join(format!("{name}.log"));
        let server_stderr_log = std::fs::File::create(log_path)?;

        let mut process = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(server_stderr_log)
            // Make sure the process is reaped on drop.
            // .kill_on_drop(true)
            .spawn()?;

        let stdin = process.stdin.take().expect("Failed to open stdin");
        let stdout = process.stdout.take().expect("Failed to open stdout");

        let payload_sender = spawn_task_stdin(stdin);

        let (response_sender_tx, response_sender_rx): (
            UnboundedSender<(Id, oneshot::Sender<RpcResponse>)>,
            _,
        ) = unbounded_channel();

        let (language_server_message_tx, language_server_message_rx) = unbounded_channel();

        tokio::spawn({
            let server_tx = payload_sender.clone();
            async move {
                language_server_message::handle_language_server_message(
                    language_server_message_rx,
                    server_tx,
                    language_server_message_handler,
                )
                .await;
            }
        });

        std::thread::spawn({
            move || {
                if let Err(err) =
                    process_server_stdout(stdout, response_sender_rx, language_server_message_tx)
                {
                    tracing::error!(?err, "Failed to process server messages, exiting...");
                }
            }
        });

        let (workspace, workspace_is_cwd) = find_workspace();
        let workspace = paths::get_normalized_path(&workspace);
        let root = find_lsp_workspace(
            doc_path
                .as_ref()
                .and_then(|x| x.parent().and_then(|x| x.to_str()))
                .unwrap_or("."),
            root_markers,
            manual_roots,
            &workspace,
            workspace_is_cwd,
        );

        // `root_uri` and `workspace_folder` can be empty in case there is no workspace
        // `root_url` can not, use `workspace` as a fallback
        let root_path = root.clone().unwrap_or_else(|| workspace.clone());
        let root_uri = root.and_then(|root| lsp::Url::from_file_path(root).ok());

        let workspace_folders = root_uri
            .clone()
            .map(|root| vec![workspace_for_uri(root)])
            .unwrap_or_default();

        let client = Self {
            id: AtomicU64::new(0),
            _name: name,
            server_tx: payload_sender,
            response_sender_tx,
            root_path,
            root_uri,
            workspace_folders: Mutex::new(workspace_folders),
            capabilities: OnceCell::new(),
            _server_process: process,
        };

        Ok(client)
    }

    pub fn name(&self) -> &str {
        &self._name
    }

    pub fn try_add_workspace(&self, root_uri: Option<lsp::Url>) -> Result<(), Error> {
        let workspace_exists = root_uri
            .clone()
            .map(|uri| self.workspace_exists(uri))
            .unwrap_or(false);

        if !workspace_exists {
            if let Some(workspace_folders_caps) = self
                .capabilities()
                .workspace
                .as_ref()
                .and_then(|cap| cap.workspace_folders.as_ref())
                .filter(|cap| cap.supported.unwrap_or(false))
            {
                self.add_workspace_folder(root_uri, &workspace_folders_caps.change_notifications)?;
            } else {
                // TODO: the server doesn't support multi workspaces, we need a new client
            }
        }

        Ok(())
    }

    pub fn add_workspace_folder(
        &self,
        root_uri: Option<lsp::Url>,
        change_notifications: &Option<lsp::OneOf<bool, String>>,
    ) -> Result<(), Error> {
        // root_uri is None just means that there isn't really any LSP workspace
        // associated with this file. For servers that support multiple workspaces
        // there is just one server so we can always just use that shared instance.
        // No need to add a new workspace root here as there is no logical root for this file
        // let the server deal with this
        let Some(root_uri) = root_uri else {
            return Ok(());
        };

        // server supports workspace folders, let's add the new root to the list
        self.workspace_folders
            .lock()
            .push(workspace_for_uri(root_uri.clone()));

        if &Some(lsp::OneOf::Left(false)) == change_notifications {
            // server specifically opted out of DidWorkspaceChange notifications
            // let's assume the server will request the workspace folders itself
            // and that we can therefore reuse the client (but are done now)
            return Ok(());
        }

        self.did_change_workspace(vec![workspace_for_uri(root_uri)], Vec::new())
    }

    pub fn workspace_exists(&self, root_uri: lsp::Url) -> bool {
        self.workspace_folders
            .lock()
            .contains(&workspace_for_uri(root_uri))
    }

    pub fn is_initialized(&self) -> bool {
        self.capabilities.get().is_some()
    }

    pub fn capabilities(&self) -> &ServerCapabilities {
        self.capabilities
            .get()
            .expect("language server not yet initialized!")
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

        tracing::trace!(request = ?rpc_request, "Sending request");
        let (request_result_tx, request_result_rx) = tokio::sync::oneshot::channel();
        // Request result will be sent back in a RpcResponse message.
        self.response_sender_tx
            .send((Id::Num(id), request_result_tx))?;
        self.server_tx.send(RpcMessage::Request(rpc_request))?;
        match request_result_rx.await? {
            RpcResponse::Success(ok) => Ok(serde_json::from_value(ok.result)?),
            RpcResponse::Failure(err) => Err(Error::ResponseFailure(err)),
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

        tracing::trace!(?notification, "Sending notification");
        self.server_tx
            .send(RpcMessage::Notification(notification))?;

        Ok(())
    }

    async fn initialize(
        &self,
        enable_snippets: bool,
        initialization_options: Option<serde_json::Value>,
    ) -> Result<lsp::InitializeResult, Error> {
        #[allow(deprecated)]
        let params = lsp::InitializeParams {
            process_id: Some(std::process::id()),
            workspace_folders: None,
            // root_path is obsolete, but some clients like pyright still use it so we specify both.
            // clients will prefer _uri if possible
            root_path: self.root_path.to_str().map(|s| s.to_string()),
            root_uri: self.root_uri.clone(),
            initialization_options,
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
        language_id: &'static str,
    ) -> Result<(), Error> {
        self.notify::<lsp::notification::DidOpenTextDocument>(lsp::DidOpenTextDocumentParams {
            text_document: lsp::TextDocumentItem {
                uri,
                language_id: language_id.to_owned(),
                version,
                text,
            },
        })
    }

    pub fn text_document_did_change(
        &self,
        text_document: lsp::VersionedTextDocumentIdentifier,
        new_text: String,
    ) -> Result<(), Error> {
        let capabilities = self.capabilities.get().ok_or(Error::Uninitialized)?;

        // Return early if the server does not support document sync.
        let sync_capabilities = match capabilities.text_document_sync {
            Some(
                lsp::TextDocumentSyncCapability::Kind(kind)
                | lsp::TextDocumentSyncCapability::Options(lsp::TextDocumentSyncOptions {
                    change: Some(kind),
                    ..
                }),
            ) => kind,
            // None | SyncOptions { changes: None }
            _ => return Ok(()),
        };

        let changes = match sync_capabilities {
            lsp::TextDocumentSyncKind::FULL => {
                vec![lsp::TextDocumentContentChangeEvent {
                    // range = None -> whole document
                    range: None,        //Some(Range)
                    range_length: None, // u64 apparently deprecated
                    text: new_text.to_string(),
                }]
            }
            lsp::TextDocumentSyncKind::INCREMENTAL => {
                // TODO: incremental changes.
                vec![lsp::TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: new_text,
                }]
            }
            lsp::TextDocumentSyncKind::NONE => return Ok(()),
            kind => unimplemented!("{:?}", kind),
        };

        self.notify::<lsp::notification::DidChangeTextDocument>(lsp::DidChangeTextDocumentParams {
            text_document,
            content_changes: changes,
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
    ) -> Result<(), Error> {
        let capabilities = self.capabilities.get().ok_or(Error::Uninitialized)?;

        let include_text = match &capabilities.text_document_sync {
            Some(lsp::TextDocumentSyncCapability::Options(lsp::TextDocumentSyncOptions {
                save:
                    Some(lsp::TextDocumentSyncSaveOptions::SaveOptions(lsp::SaveOptions {
                        include_text,
                    })),
                ..
            })) => include_text.unwrap_or(false),
            _ => false,
        };

        let text = if include_text {
            Some(std::fs::read_to_string(text_document.uri.path())?)
        } else {
            None
        };

        self.notify::<lsp::notification::DidSaveTextDocument>(lsp::DidSaveTextDocumentParams {
            text_document,
            text,
        })
    }

    pub fn did_change_configuration(&self, settings: Value) -> Result<(), Error> {
        self.notify::<lsp::notification::DidChangeConfiguration>(
            lsp::DidChangeConfigurationParams { settings },
        )
    }

    pub fn did_change_workspace(
        &self,
        added: Vec<lsp::WorkspaceFolder>,
        removed: Vec<lsp::WorkspaceFolder>,
    ) -> Result<(), Error> {
        self.notify::<lsp::notification::DidChangeWorkspaceFolders>(
            lsp::DidChangeWorkspaceFoldersParams {
                event: lsp::WorkspaceFoldersChangeEvent { added, removed },
            },
        )
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

    pub async fn text_document_formatting(
        &self,
        text_document: lsp::TextDocumentIdentifier,
        options: lsp::FormattingOptions,
        work_done_token: Option<lsp::ProgressToken>,
    ) -> Result<Vec<lsp::TextEdit>, Error> {
        let capabilities = self.capabilities.get().ok_or(Error::Uninitialized)?;

        // Return early if the server does not support formatting.
        match capabilities.document_formatting_provider {
            Some(lsp::OneOf::Left(true) | lsp::OneOf::Right(_)) => (),
            _ => return Ok(Vec::new()),
        };

        // merge FormattingOptions with 'config.format'
        // let config_format = self
        // .config
        // .as_ref()
        // .and_then(|cfg| cfg.get("format"))
        // .and_then(|fmt| HashMap::<String, lsp::FormattingProperty>::deserialize(fmt).ok());

        // let options = if let Some(mut properties) = config_format {
        // // passed in options take precedence over 'config.format'
        // properties.extend(options.properties);
        // lsp::FormattingOptions {
        // properties,
        // ..options
        // }
        // } else {
        // options
        // };

        let params = lsp::DocumentFormattingParams {
            text_document,
            options,
            work_done_progress_params: lsp::WorkDoneProgressParams { work_done_token },
        };

        Ok(self
            .request::<lsp::request::Formatting>(params)
            .await?
            .unwrap_or_default())
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
        query: impl Into<String>,
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
            query: query.into(),
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

    pub async fn rename_symbol(
        &self,
        text_document: lsp::TextDocumentIdentifier,
        position: lsp::Position,
        new_name: String,
    ) -> Result<Option<lsp::WorkspaceEdit>, Error> {
        let capabilities = self.capabilities.get().ok_or(Error::Uninitialized)?;

        // Return early if the server does not support code actions.
        match capabilities.rename_provider {
            Some(OneOf::Left(true)) | Some(OneOf::Right(_)) => (),
            _ => return Ok(None),
        }

        let params = lsp::RenameParams {
            text_document_position: lsp::TextDocumentPositionParams {
                text_document,
                position,
            },
            new_name,
            work_done_progress_params: lsp::WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        self.request::<lsp::request::Rename>(params).await
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
