use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::lsp_handler::LanguageServerMessageHandler;
use crate::stdio_server::plugin::{ActionRequest, ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::{Vim, VimError, VimResult};
use lsp::Url;
use maple_lsp::lsp;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

type LanguageId = String;

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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("lsp client not found")]
    ClientNotFound,
    #[error("document not found")]
    DocumentNotFound(usize),
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

fn find_lsp_root<'a>(filetype: &str, path: &'a Path) -> Option<&'a Path> {
    let root_markers = match filetype {
        "rust" => &["Cargo.toml"],
        _ => return None,
    };

    paths::find_project_root(path, root_markers)
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
    language_id: String,
    cursor_pos: (usize, usize),
}

/// Document associated to a buffer.
#[derive(Debug, Clone)]
struct Document {
    /// LanguageId is represented by the filetype.
    language_id: String,
    bufname: String,
    doc_id: lsp::TextDocumentIdentifier,
}

impl Document {
    fn open_new_doc(&self, client: &Arc<maple_lsp::Client>, path: &str) -> Result<(), Error> {
        let text = std::fs::read_to_string(path)?;
        let language_id = self.language_id.clone();
        client.text_document_did_open(to_url(path)?, 0, text, language_id)?;
        Ok(())
    }
}

fn to_url(path: impl AsRef<Path>) -> Result<Url, Error> {
    Url::from_file_path(path.as_ref())
        .map_err(|_| Error::InvalidUrl(format!("{}", path.as_ref().display())))
}

fn doc_id(path: impl AsRef<Path>) -> Result<lsp::TextDocumentIdentifier, Error> {
    Ok(lsp::TextDocumentIdentifier { uri: to_url(path)? })
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

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(
  id = "lsp",
  actions = [
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
    servers: HashMap<LanguageId, Arc<maple_lsp::Client>>,
    documents: HashMap<usize, Document>,
    current_goto_request: Option<GotoRequest>,
    toggle: Toggle,
}

// LspPlugin
// => manage a global list of language servers
//  => one language server serves one kind of source file.

impl LspPlugin {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            servers: HashMap::new(),
            documents: HashMap::new(),
            current_goto_request: None,
            toggle: Toggle::On,
        }
    }

    async fn on_buf_enter(&mut self, bufnr: usize) -> Result<(), Error> {
        let filetype = self.vim.getbufvar::<String>(bufnr, "&filetype").await?;

        let language_id = filetype;

        // TODO: language server config.
        // server_executable, args
        let (cmd, args, root_markers) = match language_id.as_str() {
            "rust" => (
                String::from("rust-analyzer"),
                vec![],
                vec![String::from("Cargo.toml")],
            ),
            _ => return Ok(()),
        };

        if let std::collections::hash_map::Entry::Vacant(e) = self.documents.entry(bufnr) {
            let bufname = self.vim.bufname(bufnr).await?;
            let path = self.vim.bufabspath(bufnr).await?;
            let document = Document {
                language_id: language_id.clone(),
                bufname,
                doc_id: doc_id(&path)?,
            };

            match self.servers.entry(language_id.clone()) {
                Entry::Occupied(e) => {
                    let root_uri = find_lsp_root(&language_id, path.as_ref())
                        .and_then(|p| Url::from_file_path(p).ok());

                    let client = e.get();
                    let workspace_exists = root_uri
                        .clone()
                        .map(|uri| client.workspace_exists(uri))
                        .unwrap_or(false);

                    if !workspace_exists {
                        if let Some(workspace_folders_caps) = client
                            .capabilities()
                            .workspace
                            .as_ref()
                            .and_then(|cap| cap.workspace_folders.as_ref())
                            .filter(|cap| cap.supported.unwrap_or(false))
                        {
                            client.add_workspace_folder(
                                root_uri,
                                &workspace_folders_caps.change_notifications,
                            )?;
                        } else {
                            // TODO: the server doesn't support multi workspaces, we need a new client
                        }
                    }

                    document.open_new_doc(client, &path)?;
                }
                Entry::Vacant(e) => {
                    let name = String::from(
                        cmd.rsplit_once(std::path::MAIN_SEPARATOR)
                            .map(|(_, binary)| binary)
                            .unwrap_or(&cmd),
                    );
                    let enable_snippets = false;
                    let client = maple_lsp::start_client(
                        maple_lsp::ClientParams {
                            cmd,
                            args,
                            name: name.clone(),
                            root_markers,
                            manual_roots: vec![],
                            enable_snippets,
                        },
                        Some(std::path::PathBuf::from(path.clone())),
                        LanguageServerMessageHandler::new(name, self.vim.clone()),
                    )
                    .await?;

                    document.open_new_doc(&client, &path)?;

                    e.insert(client);
                }
            }

            e.insert(document);
        }

        Ok(())
    }

    async fn on_cursor_moved(&mut self, bufnr: usize) -> VimResult<()> {
        if let Some(request_in_fly) = self.current_goto_request.take() {
            let (_bufnr, row, column) = self.vim.get_cursor_pos().await?;
            if request_in_fly.bufnr == bufnr && request_in_fly.cursor_pos != (row, column) {
                self.vim
                    .set_var("g:clap_lsp_status", "cancelling request")?;
                self.vim.redrawstatus()?;
                tokio::spawn({
                    let vim = self.vim.clone();
                    let language_id = request_in_fly.language_id.clone();
                    async move {
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        let _ = vim.set_var("g:clap_lsp_status", language_id);
                        let _ = vim.redrawstatus();
                    }
                });
                return Ok(());
            }
        }

        Ok(())
    }

    async fn on_buf_write_post(&mut self, bufnr: usize) -> Result<(), Error> {
        let document = self
            .documents
            .get_mut(&bufnr)
            .ok_or(Error::DocumentNotFound(bufnr))?;

        let client = self
            .servers
            .get(&document.language_id)
            .ok_or(Error::ClientNotFound)?;

        let path = self.vim.bufabspath(bufnr).await?;

        let new_name = self.vim.bufname(bufnr).await?;

        if !new_name.eq(&document.bufname) {
            // close old doc
            let old_doc = document.doc_id.clone();
            client.text_document_did_close(old_doc)?;

            // open new doc
            let new_doc = doc_id(&path)?;
            document.open_new_doc(client, &path)?;
            document.bufname = new_name;
            document.doc_id = new_doc;
        }

        client.text_document_did_save(document.doc_id.clone())?;

        Ok(())
    }

    fn get_doc(&self, bufnr: usize) -> Result<&Document, Error> {
        self.documents
            .get(&bufnr)
            .ok_or(Error::DocumentNotFound(bufnr))
    }

    async fn goto_impl(&mut self, goto: Goto) -> Result<(), Error> {
        let bufnr = self.vim.bufnr("").await?;
        let document = self.get_doc(bufnr)?;

        let client = self
            .servers
            .get(&document.language_id)
            .ok_or(Error::ClientNotFound)?;

        let path = self.vim.bufabspath(bufnr).await?;
        let (bufnr, row, column) = self.vim.get_cursor_pos().await?;
        let position = lsp::Position {
            line: row as u32 - 1,
            character: column as u32 - 1,
        };
        let text_document = lsp::TextDocumentIdentifier {
            uri: Url::from_file_path(&path).map_err(|_| Error::InvalidUrl(path))?,
        };

        tracing::debug!(bufnr, doc = ?text_document, "Calling goto, position: {position:?}");

        let now = std::time::Instant::now();

        self.vim
            .set_var("g:clap_lsp_status", format!("requesting {goto:?}"))?;
        self.vim.redrawstatus()?;
        self.current_goto_request.replace(GotoRequest {
            bufnr,
            language_id: document.language_id.clone(),
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
                let include_declaration = false;
                client
                    .goto_reference(text_document, position, include_declaration, None)
                    .await
                    .map(|res| res.unwrap_or_default())
            }
        };

        let locations = match locations_result {
            Ok(locations) => locations,
            Err(maple_lsp::Error::RequestFailure(request_failure)) => {
                self.vim
                    .echo_message(format!("request_failure: {request_failure:?}"))?;
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };

        tracing::debug!("goto-definition , elapsed: {}ms", now.elapsed().as_millis());

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

    async fn document_symbols(&mut self) -> Result<(), Error> {
        let bufnr = self.vim.bufnr("").await?;
        let document = self.get_doc(bufnr)?;

        let client = self
            .servers
            .get(&document.language_id)
            .ok_or(Error::ClientNotFound)?;

        if !client.is_initialized() {
            self.vim
                .echo_message("language server not yet initialized")?;
            return Ok(());
        }

        let doc_id = document.doc_id.clone();

        let document_symbol_response = client.document_symbols(doc_id.clone()).await?;

        let symbols = match document_symbol_response {
            Some(symbols) => symbols,
            None => return Ok(()),
        };

        // lsp has two ways to represent symbols (flat/nested)
        // convert the nested variant to flat, so that we have a homogeneous list
        let symbols = match symbols {
            lsp::DocumentSymbolResponse::Flat(symbols) => symbols,
            lsp::DocumentSymbolResponse::Nested(symbols) => {
                let mut flat_symbols = Vec::new();
                for symbol in symbols {
                    nested_to_flat(&mut flat_symbols, &doc_id, symbol)
                }
                flat_symbols
            }
        };

        let symbols = symbols
            .into_iter()
            .map(|symbol| format!("{} {:?}", symbol.name, symbol.kind))
            .collect::<Vec<_>>();

        tracing::debug!("symbols: {symbols:?}");

        // use crate::stdio_server::provider::lsp::{LspProvider, LspSource};
        // let _provider = LspProvider::new(true, 75, LspSource::DocumentSymbols(symbols));

        Ok(())
    }

    async fn workspace_symbols(&mut self) -> Result<(), Error> {
        let bufnr = self.vim.bufnr("").await?;
        let document = self.get_doc(bufnr)?;

        let client = self
            .servers
            .get(&document.language_id)
            .ok_or(Error::ClientNotFound)?;

        if !client.is_initialized() {
            self.vim
                .echo_message("language server not yet initialized")?;
            return Ok(());
        }

        // Use empty query to fetch all workspace symbols.
        let workspace_symbol_response = client.workspace_symbols("".to_string()).await?;

        let symbols = match workspace_symbol_response {
            Some(symbols) => symbols,
            None => return Ok(()),
        };

        tracing::debug!("workspace symbols: {symbols:?}");

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
                self.on_buf_enter(bufnr).await?;
            }
            BufWritePost => {
                self.on_buf_write_post(bufnr).await?;
            }
            BufDelete => {
                if let Some(doc) = self.documents.remove(&bufnr) {
                    let client = self
                        .servers
                        .get(&doc.language_id)
                        .ok_or(Error::ClientNotFound)?;

                    client
                        .text_document_did_close(doc.doc_id)
                        .map_err(Error::Lsp)?;
                }
            }
            CursorMoved => {
                self.on_cursor_moved(bufnr).await?;
            }
            event => return Err(PluginError::UnhandledEvent(event)),
        }

        Ok(())
    }

    async fn handle_action(&mut self, action: ActionRequest) -> Result<(), PluginError> {
        let ActionRequest { method, params: _ } = action;
        match self.parse_action(method)? {
            LspAction::Toggle => {
                match self.toggle {
                    Toggle::On => {}
                    Toggle::Off => {
                        let bufnr = self.vim.bufnr("").await?;
                        self.on_buf_enter(bufnr).await?;
                    }
                }
                self.toggle.switch();
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
