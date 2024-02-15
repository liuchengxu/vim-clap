use crate::stdio_server::diagnostics_worker::WorkerMessage as DiagnosticsWorkerMessage;
use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::lsp::{
    find_lsp_root, language_id_from_path, LanguageServerMessageHandler,
};
use crate::stdio_server::plugin::{ClapPlugin, PluginAction, PluginError, Toggle};
use crate::stdio_server::provider::lsp::{set_lsp_source, LspSource};
use crate::stdio_server::vim::{Vim, VimError, VimResult};
use lsp::Url;
use maple_lsp::lsp;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("lsp client not found")]
    ClientNotFound,
    #[error("language id not found for buffer {0}")]
    LanguageIdNotFound(usize),
    #[error("language config not found: {0}")]
    UnsupportedLanguage(LanguageId),
    #[error("buffer not attached")]
    BufferNotAttached(usize),
    #[error("invalid Url: {0}")]
    InvalidUrl(String),
    #[error(transparent)]
    Vim(#[from] VimError),
    #[error(transparent)]
    Lsp(#[from] maple_lsp::Error),
    #[error(transparent)]
    JsonRpc(#[from] rpc::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::RpcError),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Path(#[from] std::path::StripPrefixError),
    #[error(transparent)]
    FromUtf8(#[from] std::string::FromUtf8Error),
}

#[derive(serde::Serialize)]
struct FileLocation {
    /// Absolute file path.
    path: String,
    /// 1-based.
    row: u32,
    /// 1-based
    column: u32,
    text: String,
}

#[derive(Debug)]
enum Goto {
    Definition,
    Declaration,
    TypeDefinition,
    Implementation,
    Reference,
}

#[derive(Debug, Clone)]
struct GotoRequest {
    bufnr: usize,
    language_id: LanguageId,
    cursor_pos: (usize, usize),
}

type LanguageId = &'static str;

/// Represents an attached buffer.
#[derive(Debug, Clone)]
struct Buffer {
    bufname: String,
    doc_id: lsp::TextDocumentIdentifier,
    language_id: LanguageId,
}

fn to_url(path: impl AsRef<Path>) -> Result<Url, Error> {
    Url::from_file_path(path.as_ref())
        .map_err(|_| Error::InvalidUrl(format!("{}", path.as_ref().display())))
}

fn to_file_path(uri: lsp::Url) -> Result<PathBuf, Error> {
    uri.to_file_path()
        .map_err(|()| Error::InvalidUrl(format!("uri {uri} is not a file path")))
}

fn doc_id(path: impl AsRef<Path>) -> Result<lsp::TextDocumentIdentifier, Error> {
    Ok(lsp::TextDocumentIdentifier { uri: to_url(path)? })
}

fn open_new_doc(
    client: &Arc<maple_lsp::Client>,
    language_id: LanguageId,
    path: &str,
) -> Result<(), Error> {
    let text = std::fs::read_to_string(path)?;
    client.text_document_did_open(to_url(path)?, 0, text, language_id)?;
    Ok(())
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LanguageConfig {
    /// c-sharp, rust, tsx
    #[serde(rename = "name")]
    pub language_id: String,

    /// see the table under https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocumentItem
    /// csharp, rust, typescriptreact, for the language-server
    #[serde(rename = "language-id")]
    pub language_server_language_id: Option<String>,

    /// these indicate project roots <.git, Cargo.toml>
    #[serde(default)]
    pub root_markers: Vec<String>,
}

// TODO: support more languages.
fn get_language_config(language_id: LanguageId) -> Option<maple_lsp::LanguageConfig> {
    let language_config = match language_id {
        "rust" => maple_lsp::LanguageConfig {
            cmd: String::from("rust-analyzer"),
            args: vec![],
            root_markers: vec![String::from("Cargo.toml")],
        },
        "go" => maple_lsp::LanguageConfig {
            cmd: String::from("gopls"),
            args: vec![],
            root_markers: vec![String::from("go.mod")],
        },
        _ => return None,
    };

    Some(language_config)
}

fn preprocess_text_edits(text_edits: Vec<lsp::TextEdit>) -> Vec<lsp::TextEdit> {
    let mut text_edits = text_edits;

    // Simply follows the logic in
    // https://github.com/prabirshrestha/vim-lsp/blob/d36f381dc8f39a9b86d66ef84c2ebbb7516d91d6/autoload/lsp/utils/text_edit.vim#L160
    text_edits.iter_mut().for_each(|text_edit| {
        let start = text_edit.range.start;
        let end = text_edit.range.end;

        if start.line > end.line || (start.line == end.line && start.character > end.character) {
            text_edit.range = lsp::Range {
                start: end,
                end: start,
            };
        }
    });

    // Sort edits by start range, since some LSPs (Omnisharp) send them
    // in reverse order.
    text_edits.sort_unstable_by_key(|edit| edit.range.start);

    text_edits.reverse();

    text_edits
}

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(
  id = "lsp",
  actions = [
    "__reload",
    "__detach",
    "__did_change",
    "format",
    "rename",
    "document-symbols",
    "workspace-symbols",
    "goto-definition",
    "goto-declaration",
    "goto-type-definition",
    "goto-implementation",
    "goto-reference",
    "toggle",
  ]
)]
pub struct LspPlugin {
    vim: Vim,
    /// Active language server clients.
    clients: HashMap<LanguageId, Arc<maple_lsp::Client>>,
    /// Documents being tracked, keyed by buffer number.
    attached_buffers: HashMap<usize, Buffer>,
    /// Goto request in fly.
    current_goto_request: Option<GotoRequest>,
    diagnostics_worker_msg_sender: UnboundedSender<DiagnosticsWorkerMessage>,
    toggle: Toggle,
}

impl LspPlugin {
    pub fn new(
        vim: Vim,
        diagnostics_worker_msg_sender: UnboundedSender<DiagnosticsWorkerMessage>,
    ) -> Self {
        Self {
            vim,
            clients: HashMap::new(),
            attached_buffers: HashMap::new(),
            current_goto_request: None,
            diagnostics_worker_msg_sender,
            toggle: Toggle::On,
        }
    }

    async fn buffer_attach(&mut self, bufnr: usize) -> Result<(), Error> {
        let path = self.vim.bufabspath(bufnr).await?;

        let language_id = language_id_from_path(&path).ok_or(Error::LanguageIdNotFound(bufnr))?;
        let language_config =
            get_language_config(language_id).ok_or(Error::UnsupportedLanguage(language_id))?;

        if let Entry::Vacant(e) = self.attached_buffers.entry(bufnr) {
            let bufname = self.vim.bufname(bufnr).await?;
            let buffer = Buffer {
                language_id,
                bufname,
                doc_id: doc_id(&path)?,
            };

            match self.clients.entry(language_id) {
                Entry::Occupied(e) => {
                    let root_uri = find_lsp_root(language_id, path.as_ref())
                        .and_then(|p| Url::from_file_path(p).ok());
                    let client = e.get();
                    client.try_add_workspace(root_uri)?;
                    open_new_doc(client, buffer.language_id, &path)?;
                }
                Entry::Vacant(e) => {
                    let enable_snippets = false;
                    let name = language_config.server_name();
                    let client = maple_lsp::start_client(
                        maple_lsp::ClientParams {
                            language_config,
                            manual_roots: vec![],
                            enable_snippets,
                        },
                        name.clone(),
                        Some(PathBuf::from(path.clone())),
                        LanguageServerMessageHandler::new(
                            name,
                            self.vim.clone(),
                            self.diagnostics_worker_msg_sender.clone(),
                        ),
                    )
                    .await?;

                    open_new_doc(&client, buffer.language_id, &path)?;

                    e.insert(client);
                }
            }

            self.vim.exec("clap#plugin#lsp#buf_attach", [bufnr])?;

            e.insert(buffer);
        }

        Ok(())
    }

    fn buffer_detach(&mut self, bufnr: usize) -> Result<(), Error> {
        if let Some(buffer) = self.attached_buffers.remove(&bufnr) {
            let client = self
                .clients
                .get(&buffer.language_id)
                .ok_or(Error::ClientNotFound)?;

            client
                .text_document_did_close(buffer.doc_id)
                .map_err(Error::Lsp)?;
        }
        Ok(())
    }

    async fn text_document_did_change(
        &self,
        (bufnr, changedtick, _firstline, _lastline, _new_lastline): (
            usize,
            i32,
            usize,
            usize,
            usize,
        ),
    ) -> Result<(), Error> {
        let document = self.get_buffer(bufnr)?;

        let client = self
            .clients
            .get(&document.language_id)
            .ok_or(Error::ClientNotFound)?;

        // TODO: incremental changes
        let new_text = self.vim.getbufline(bufnr, 1, '$').await?.join("\n");

        let _ = client.text_document_did_change(
            lsp::VersionedTextDocumentIdentifier {
                uri: document.doc_id.uri.clone(),
                version: changedtick,
            },
            new_text,
        );

        Ok(())
    }

    async fn text_document_did_save(&mut self, bufnr: usize) -> Result<(), Error> {
        let buffer = self
            .attached_buffers
            .get_mut(&bufnr)
            .ok_or(Error::BufferNotAttached(bufnr))?;

        let client = self
            .clients
            .get(&buffer.language_id)
            .ok_or(Error::ClientNotFound)?;

        let new_name = self.vim.bufname(bufnr).await?;

        // Reload the document.
        if !new_name.eq(&buffer.bufname) {
            // Close old doc.
            let old_doc = buffer.doc_id.clone();
            client.text_document_did_close(old_doc)?;

            // Open new doc.
            let path = self.vim.bufabspath(bufnr).await?;
            let new_doc = doc_id(&path)?;
            open_new_doc(client, buffer.language_id, &path)?;
            buffer.bufname = new_name;
            buffer.doc_id = new_doc;
        }

        client.text_document_did_save(buffer.doc_id.clone())?;

        Ok(())
    }

    async fn on_cursor_moved(&mut self, bufnr: usize) -> VimResult<()> {
        if let Some(request_in_fly) = self.current_goto_request.take() {
            let (_bufnr, row, column) = self.vim.get_cursor_pos().await?;
            if request_in_fly.bufnr == bufnr && request_in_fly.cursor_pos != (row, column) {
                self.vim.update_lsp_status("cancelling request")?;
                tokio::spawn({
                    let vim = self.vim.clone();
                    let language_id = request_in_fly.language_id;
                    async move {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        let _ = vim.update_lsp_status(language_id);
                    }
                });
                return Ok(());
            }
        }

        Ok(())
    }

    fn get_buffer(&self, bufnr: usize) -> Result<&Buffer, Error> {
        self.attached_buffers
            .get(&bufnr)
            .ok_or(Error::BufferNotAttached(bufnr))
    }

    async fn get_cursor_lsp_position(
        &self,
        bufnr: usize,
        filepath: &Path,
    ) -> Result<Option<lsp::Position>, Error> {
        let line = self.vim.line(".").await?;
        let col = self.vim.col(".").await?;
        let lines = self.vim.getbufline(bufnr, line, line).await?;

        let maybe_character_index = if lines.is_empty() {
            // Buffer may not be loaded, read the local file directly.
            let Some(line) = utils::read_line_at(filepath, line)? else {
                return Ok(None);
            };
            utils::char_index_for(&line, col - 1)
        } else {
            utils::char_index_for(&lines[0], col - 1)
        };

        let Some(character) = maybe_character_index else {
            return Ok(None);
        };

        let cursor_lsp_position = lsp::Position {
            line: line as u32 - 1,
            character: character as u32,
        };

        Ok(Some(cursor_lsp_position))
    }

    async fn goto_impl(&mut self, goto: Goto) -> Result<(), Error> {
        let bufnr = self.vim.bufnr("").await?;
        let document = self.get_buffer(bufnr)?;

        let client = self
            .clients
            .get(&document.language_id)
            .ok_or(Error::ClientNotFound)?;

        let path = self.vim.bufabspath(bufnr).await?;
        let (bufnr, row, column) = self.vim.get_cursor_pos().await?;
        let position = lsp::Position {
            line: row as u32 - 1,
            character: column as u32 - 1,
        };
        let text_document = doc_id(&path)?;

        tracing::debug!(bufnr, doc = ?text_document, "Calling goto, position: {position:?}");

        self.vim.update_lsp_status(format!("requesting {goto:?}"))?;
        self.current_goto_request.replace(GotoRequest {
            bufnr,
            language_id: document.language_id,
            cursor_pos: (row, column),
        });

        let locations_result = match goto {
            Goto::Definition => client.goto_definition(text_document, position, None).await,
            Goto::Declaration => client.goto_declaration(text_document, position, None).await,
            Goto::TypeDefinition => {
                client
                    .goto_type_definition(text_document, position, None)
                    .await
            }
            Goto::Implementation => {
                client
                    .goto_implementation(text_document, position, None)
                    .await
            }
            Goto::Reference => {
                // TODO: configurable include_declaration flag.
                let include_declaration = false;
                client
                    .goto_reference(text_document, position, include_declaration, None)
                    .await
                    .map(|res| res.unwrap_or_default())
            }
        };

        let locations = match locations_result {
            Ok(locations) => locations,
            Err(maple_lsp::Error::ResponseFailure(request_failure)) => {
                self.vim
                    .echo_message(format!("request_failure: {request_failure:?}"))?;
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };

        let (_bufnr, new_row, new_column) = self.vim.get_cursor_pos().await?;
        if (new_row, new_column) != (row, column) {
            self.current_goto_request.take();
            self.vim.update_lsp_status("cancelling request")?;
            return Ok(());
        }

        if locations.is_empty() {
            self.vim.echo_message(format!("{goto:?} not found"))?;
            return Ok(());
        }

        let locations = locations
            .into_iter()
            .map(|loc| {
                let path = loc.uri.path();
                let row = loc.range.start.line + 1;
                let column = loc.range.start.character + 1;
                let text = utils::read_line_at(path, row as usize)
                    .ok()
                    .flatten()
                    .unwrap_or_default();

                FileLocation {
                    path: path.to_string(),
                    row,
                    column,
                    text,
                }
            })
            .collect::<Vec<_>>();

        self.vim.exec(
            "clap#plugin#lsp#handle_locations",
            (format!("{goto:?}"), locations),
        )?;
        self.vim.update_lsp_status("rust-analyzer")?;
        self.current_goto_request.take();

        Ok(())
    }

    fn open_picker(&self, lsp_source: LspSource) -> VimResult<()> {
        let title = match lsp_source {
            LspSource::DocumentSymbols(_) => "documentSymbols",
            LspSource::WorkspaceSymbols(_) => "workspaceSymbols",
            LspSource::Empty => unreachable!("source must not be empty to open"),
        };
        set_lsp_source(lsp_source);
        self.vim.exec("clap#plugin#lsp#open_picker", [title])
    }

    async fn text_document_format(&mut self) -> Result<(), Error> {
        let bufnr = self.vim.bufnr("").await?;
        let buffer = self.get_buffer(bufnr)?;

        let client = self
            .clients
            .get(&buffer.language_id)
            .ok_or(Error::ClientNotFound)?;

        let doc_id = buffer.doc_id.clone();

        let text_edits = client
            .text_document_formatting(
                doc_id.clone(),
                lsp::FormattingOptions {
                    tab_size: self
                        .vim
                        .call::<u32>("clap#plugin#lsp#tab_size", bufnr)
                        .await?,
                    insert_spaces: self.vim.getbufvar::<usize>(bufnr, "&expandtab").await? == 1,
                    ..Default::default()
                },
                None,
            )
            .await?;

        if !text_edits.is_empty() {
            let text_edits = preprocess_text_edits(text_edits);

            let filepath = doc_id.uri.to_file_path().map_err(|()| {
                Error::InvalidUrl(format!("uri: {} is not a file path", doc_id.uri))
            })?;

            let Ok(Some(cursor_lsp_position)) =
                self.get_cursor_lsp_position(bufnr, &filepath).await
            else {
                return Ok(());
            };

            self.vim.exec(
                "clap#lsp#text_edit#apply_text_edits",
                (filepath, text_edits, cursor_lsp_position),
            )?;
        }

        Ok(())
    }

    async fn process_text_document_edit(
        &self,
        bufnr: usize,
        doc_edit: lsp::TextDocumentEdit,
    ) -> Result<(), Error> {
        let uri = doc_edit.text_document.uri;

        let edits = doc_edit
            .edits
            .into_iter()
            .map(|edit| match edit {
                lsp::OneOf::Left(edit) => edit,
                lsp::OneOf::Right(annotated_edit) => annotated_edit.text_edit,
            })
            .collect();

        let text_edits = preprocess_text_edits(edits);

        let filepath = to_file_path(uri)?;

        let Ok(Some(cursor_lsp_position)) = self.get_cursor_lsp_position(bufnr, &filepath).await
        else {
            return Ok(());
        };

        self.vim.exec(
            "clap#lsp#text_edit#apply_text_edits",
            (filepath, text_edits, cursor_lsp_position),
        )?;

        Ok(())
    }

    async fn rename_symbol(&mut self) -> Result<(), Error> {
        let bufnr = self.vim.bufnr("").await?;
        let document = self.get_buffer(bufnr)?;

        let client = self
            .clients
            .get(&document.language_id)
            .ok_or(Error::ClientNotFound)?;

        let doc_id = document.doc_id.clone();

        let Ok(Some(cursor_lsp_position)) = self
            .get_cursor_lsp_position(bufnr, &to_file_path(document.doc_id.uri.clone())?)
            .await
        else {
            return Ok(());
        };

        let maybe_workspace_edit = match client
            .rename_symbol(doc_id.clone(), cursor_lsp_position, "NewName".into())
            .await
        {
            Ok(res) => res,
            Err(maple_lsp::Error::ResponseFailure(failure)) => {
                self.vim.echo_message(failure.error.message)?;
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };

        if let Some(workspace_edit) = maybe_workspace_edit {
            if let Some(document_changes) = workspace_edit.document_changes {
                match document_changes {
                    lsp::DocumentChanges::Edits(edits) => {
                        for edit in edits {
                            self.process_text_document_edit(bufnr, edit).await?;
                        }
                    }
                    lsp::DocumentChanges::Operations(operations) => {
                        for operation in operations {
                            match operation {
                                lsp::DocumentChangeOperation::Op(op) => {
                                    tracing::debug!("TODO: handle op {op:?}");
                                }
                                lsp::DocumentChangeOperation::Edit(edit) => {
                                    self.process_text_document_edit(bufnr, edit).await?;
                                }
                            }
                        }
                    }
                }
            } else if let Some(changes) = workspace_edit.changes {
                for (uri, edits) in changes {
                    let filepath = to_file_path(uri)?;
                    let text_edits = preprocess_text_edits(edits);

                    self.vim.exec(
                        "clap#lsp#text_edit#apply_text_edits",
                        (filepath, text_edits, cursor_lsp_position),
                    )?;
                }
            }
        }

        Ok(())
    }

    async fn document_symbols(&mut self) -> Result<(), Error> {
        fn nested_to_flat(
            list: &mut Vec<lsp::SymbolInformation>,
            file: &lsp::TextDocumentIdentifier,
            symbol: lsp::DocumentSymbol,
        ) {
            #[allow(deprecated)]
            list.push(lsp::SymbolInformation {
                name: symbol.name,
                kind: symbol.kind,
                tags: symbol.tags,
                deprecated: symbol.deprecated,
                location: lsp::Location::new(file.uri.clone(), symbol.selection_range),
                container_name: None,
            });
            for child in symbol.children.into_iter().flatten() {
                nested_to_flat(list, file, child);
            }
        }

        let bufnr = self.vim.bufnr("").await?;
        let buffer = self.get_buffer(bufnr)?;

        let client = self
            .clients
            .get(&buffer.language_id)
            .ok_or(Error::ClientNotFound)?;

        let Some(symbols) = client.document_symbols(buffer.doc_id.clone()).await? else {
            return Ok(());
        };

        // lsp has two ways to represent symbols (flat/nested)
        // convert the nested variant to flat, so that we have a homogeneous list
        let symbols = match symbols {
            lsp::DocumentSymbolResponse::Flat(symbols) => symbols,
            lsp::DocumentSymbolResponse::Nested(symbols) => {
                let mut flat_symbols = Vec::new();
                for symbol in symbols {
                    nested_to_flat(&mut flat_symbols, &buffer.doc_id, symbol)
                }
                flat_symbols
            }
        };

        let uri = buffer.doc_id.uri.clone();
        self.open_picker(LspSource::DocumentSymbols((uri, symbols)))?;

        Ok(())
    }

    async fn workspace_symbols(&mut self) -> Result<(), Error> {
        #[allow(deprecated)]
        fn into_symbol_information(symbol: lsp::WorkspaceSymbol) -> lsp::SymbolInformation {
            lsp::SymbolInformation {
                name: symbol.name,
                kind: symbol.kind,
                tags: symbol.tags,
                deprecated: None,
                location: match symbol.location {
                    lsp::OneOf::Left(location) => location,
                    lsp::OneOf::Right(workspace_location) => lsp::Location {
                        uri: workspace_location.uri,
                        range: Default::default(),
                    },
                },
                container_name: symbol.container_name,
            }
        }

        let bufnr = self.vim.bufnr("").await?;
        let buffer = self.get_buffer(bufnr)?;

        let client = self
            .clients
            .get(&buffer.language_id)
            .ok_or(Error::ClientNotFound)?;

        // Use empty query to fetch all workspace symbols.
        let Some(symbols) = client.workspace_symbols("").await? else {
            return Ok(());
        };

        let symbols = match symbols {
            lsp::WorkspaceSymbolResponse::Flat(symbols) => symbols,
            lsp::WorkspaceSymbolResponse::Nested(symbols) => {
                symbols.into_iter().map(into_symbol_information).collect()
            }
        };

        self.open_picker(LspSource::WorkspaceSymbols(symbols))?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapPlugin for LspPlugin {
    #[maple_derive::subscriptions]
    async fn handle_autocmd(&mut self, autocmd: AutocmdEvent) -> Result<(), PluginError> {
        use AutocmdEventType::{BufDelete, BufEnter, BufNewFile, BufWritePost, CursorMoved};

        if self.toggle.is_off() {
            return Ok(());
        }

        let (autocmd_event_type, params) = autocmd;

        let bufnr = params.parse_bufnr()?;

        match autocmd_event_type {
            BufNewFile => {}
            BufEnter => {
                self.buffer_attach(bufnr).await?;
            }
            BufWritePost => {
                self.text_document_did_save(bufnr).await?;
            }
            BufDelete => {
                self.buffer_detach(bufnr)?;
            }
            CursorMoved => {
                self.on_cursor_moved(bufnr).await?;
            }
            event => return Err(PluginError::UnhandledEvent(event)),
        }

        Ok(())
    }

    async fn handle_action(&mut self, action: PluginAction) -> Result<(), PluginError> {
        let PluginAction { method, params } = action;
        match self.parse_action(method)? {
            LspAction::__DidChange => self.text_document_did_change(params.parse()?).await?,
            LspAction::__Reload => {
                tracing::debug!("reload buffer: {params:?}");
            }
            LspAction::__Detach => {
                tracing::debug!("detach buffer: {params:?}");
            }
            LspAction::Toggle => {
                match self.toggle {
                    Toggle::On => {}
                    Toggle::Off => {
                        let bufnr = self.vim.bufnr("").await?;
                        self.buffer_attach(bufnr).await?;
                    }
                }
                self.toggle.switch();
            }
            LspAction::Format => {
                self.text_document_format().await?;
            }
            LspAction::Rename => {
                self.rename_symbol().await?;
            }
            LspAction::GotoDefinition => {
                self.goto_impl(Goto::Definition).await?;
            }
            LspAction::GotoDeclaration => {
                self.goto_impl(Goto::Declaration).await?;
            }
            LspAction::GotoTypeDefinition => {
                self.goto_impl(Goto::TypeDefinition).await?;
            }
            LspAction::GotoImplementation => {
                self.goto_impl(Goto::Implementation).await?;
            }
            LspAction::GotoReference => {
                self.goto_impl(Goto::Reference).await?;
            }
            LspAction::DocumentSymbols => {
                self.document_symbols().await?;
            }
            LspAction::WorkspaceSymbols => {
                self.workspace_symbols().await?;
            }
        }

        Ok(())
    }
}
