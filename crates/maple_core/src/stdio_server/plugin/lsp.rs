use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::lsp_handler::LanguageServerMessageHandler;
use crate::stdio_server::plugin::{ActionRequest, ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::{Vim, VimError, VimResult};
use lsp::Url;
use maple_lsp::lsp;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

fn find_project_root<'a>(filetype: &str, path: &'a Path) -> Option<&'a Path> {
    let root_markers = match filetype {
        "rust" => &["Cargo.toml"],
        _ => return None,
    };

    paths::find_project_root(path, root_markers)
}

enum Goto {
    Definition,
    Declaration,
    TypeDefinition,
    Implementation,
    Reference,
}

impl Goto {
    fn name(&self) -> &'static str {
        match self {
            Self::Definition => "definition",
            Self::Declaration => "declaration",
            Self::TypeDefinition => "typeDefinition",
            Self::Implementation => "implementation",
            Self::Reference => "reference",
        }
    }
}

#[derive(Debug, Clone)]
struct GotoRequest {
    bufnr: usize,
    cursor_pos: (usize, usize),
}

/// Client per buffer.
#[derive(Debug, Clone)]
struct Document {
    language_id: String,
    bufname: String,
    doc_id: lsp::TextDocumentIdentifier,
}

impl Document {
    fn open_new_doc(&mut self, client: &Arc<maple_lsp::Client>, path: &str) -> Result<(), Error> {
        let capabilities = client.capabilities();
        let include_text = match &capabilities.text_document_sync {
            Some(lsp::TextDocumentSyncCapability::Options(lsp::TextDocumentSyncOptions {
                save: Some(options),
                ..
            })) => match options {
                lsp::TextDocumentSyncSaveOptions::Supported(true) => false,
                lsp::TextDocumentSyncSaveOptions::SaveOptions(lsp::SaveOptions {
                    include_text,
                }) => include_text.unwrap_or(false),
                _ => false,
            },
            _ => false,
        };

        let text = if include_text {
            std::fs::read_to_string(&path)?
        } else {
            String::default()
        };

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

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(
  id = "lsp",
  actions = [
    "debug",
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
    clients: HashMap<LanguageId, Arc<maple_lsp::Client>>,
    documents: HashMap<usize, Document>,
    current_goto_request: Option<GotoRequest>,
    toggle: Toggle,
}

impl LspPlugin {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            clients: HashMap::new(),
            documents: HashMap::new(),
            current_goto_request: None,
            toggle: Toggle::On,
        }
    }

    async fn on_buf_enter(&mut self, bufnr: usize) -> Result<(), Error> {
        let filetype = self.vim.getbufvar::<String>(bufnr, "&filetype").await?;

        let (server_binary, args) = match filetype.as_str() {
            "rust" => ("rust-analyzer", vec![]),
            _ => return Ok(()),
        };

        let path = self.vim.bufabspath(bufnr).await?;
        let path = PathBuf::from(path);
        let Some(project_root) = find_project_root(&filetype, &path) else {
            return Ok(());
        };

        let language_id = filetype;

        if !self.documents.contains_key(&bufnr) {
            if !self.clients.contains_key(&language_id) {
                let client = maple_lsp::start_client(
                    server_binary,
                    &args,
                    project_root,
                    LanguageServerMessageHandler::new(server_binary.to_string(), self.vim.clone()),
                    false,
                )?;

                self.clients.insert(language_id.clone(), client.clone());
            }
            let bufname = self.vim.bufname(bufnr).await?;
            let path = self.vim.bufabspath(bufnr).await?;
            let doc = Document {
                language_id,
                bufname,
                doc_id: lsp::TextDocumentIdentifier { uri: to_url(path)? },
            };
            self.documents.insert(bufnr, doc);
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
                return Ok(());
            }
        }

        Ok(())
    }

    async fn on_buf_write_post(&mut self, bufnr: usize) -> Result<(), Error> {
        let document = self
            .documents
            .get_mut(&bufnr)
            .ok_or(Error::ClientNotFound)?;

        let client = self
            .clients
            .get(&document.language_id)
            .ok_or(Error::DocumentNotFound(bufnr))?;

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

        let text = if client.include_text_on_save() {
            Some(std::fs::read_to_string(&path)?)
        } else {
            None
        };
        client.text_document_did_save(document.doc_id.clone(), text)?;

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
            .clients
            .get(&document.language_id)
            .ok_or(Error::ClientNotFound)?;

        if !client.is_initialized() {
            self.vim
                .echo_message("language server not yet initialized")?;
            return Ok(());
        }

        let path = self.vim.bufabspath(bufnr).await?;
        let (bufnr, row, column) = self.vim.get_cursor_pos().await?;
        let position = lsp::Position {
            line: row as u32 - 1,
            character: column as u32 - 1,
        };
        let text_document = lsp::TextDocumentIdentifier {
            uri: Url::from_file_path(&path).map_err(|_| Error::InvalidUrl(path))?,
        };

        tracing::debug!("starting goto");

        let now = std::time::Instant::now();

        self.vim
            .set_var("g:clap_lsp_status", format!("requesting {}", goto.name()))?;
        self.vim.redrawstatus()?;
        self.current_goto_request.replace(GotoRequest {
            bufnr,
            cursor_pos: (row, column),
        });

        let locations = match goto {
            Goto::Definition => {
                client
                    .goto_definition(text_document, position, None)
                    .await?
            }
            Goto::Declaration => {
                client
                    .goto_declaration(text_document, position, None)
                    .await?
            }
            Goto::TypeDefinition => {
                client
                    .goto_type_definition(text_document, position, None)
                    .await?
            }
            Goto::Implementation => {
                client
                    .goto_implementation(text_document, position, None)
                    .await?
            }
            Goto::Reference => {
                let include_declaration = false;
                client
                    .goto_reference(text_document, position, include_declaration, None)
                    .await?
                    .unwrap_or_default()
            }
        };

        tracing::debug!("goto-definition , elapsed: {}ms", now.elapsed().as_millis());

        let (_bufnr, new_row, new_column) = self.vim.get_cursor_pos().await?;
        if (new_row, new_column) != (row, column) {
            self.current_goto_request.take();
            self.vim.update_lsp_status("cancelling request")?;
            return Ok(());
        }

        if locations.is_empty() {
            self.vim
                .echo_message(format!("{} not found", goto.name()))?;
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

        self.vim
            .exec("clap#plugin#lsp#handle_locations", (goto.name(), locations))?;
        self.vim.update_lsp_status("rust-analyzer")?;
        self.current_goto_request.take();

        Ok(())
    }

    async fn document_symbols(&mut self) -> Result<(), Error> {
        let bufnr = self.vim.bufnr("").await?;
        let document = self.get_doc(bufnr)?;

        let client = self
            .clients
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
            .clients
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
            BufNewFile => {
                tracing::debug!("============ [lsp] BufNewFile: {bufnr}");
            }
            BufEnter => self.on_buf_enter(bufnr).await?,
            BufWritePost => self.on_buf_write_post(bufnr).await?,
            BufDelete => {}
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
            LspAction::Debug => {}
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
