use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::lsp_handler::LanguageServerMessageHandler;
use crate::stdio_server::plugin::{ActionRequest, ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::{Vim, VimError, VimResult};
use lsp::{ServerCapabilities, Url};
use maple_lsp::{lsp, Client};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

type BufferInfo = Vec<u8>;
type LanguageId = String;

#[derive(serde::Serialize)]
struct FileLocation {
    path: String,
    /// 1-based.
    row: u32,
    /// 1-based
    column: u32,
}

impl FileLocation {
    fn from_lsp_location(loc: &lsp::Location) -> Self {
        Self {
            path: loc.uri.path().to_string(),
            row: loc.range.start.line + 1,
            column: loc.range.start.character + 1,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Other(String),
    #[error("lsp client not found")]
    ClientNotFound,
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

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(
  id = "lsp",
  actions = [
    "debug",
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
    bufs: HashMap<usize, BufferInfo>,
    clients: HashMap<LanguageId, Arc<Client>>,
    capabilities: HashMap<LanguageId, ServerCapabilities>,
    toggle: Toggle,
}

impl LspPlugin {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            bufs: HashMap::new(),
            clients: HashMap::new(),
            capabilities: HashMap::new(),
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

        if !self.clients.contains_key(&language_id) {
            let client = Arc::new(Client::new(
                server_binary,
                &args,
                project_root,
                LanguageServerMessageHandler::new(self.vim.clone()),
            )?);

            let now = std::time::Instant::now();
            let enable_snippets = false;
            let initialize_result = client.initialize(enable_snippets).await?;

            client.notify::<lsp::notification::Initialized>(lsp::InitializedParams {})?;

            tracing::debug!(
                "Client initialized, elapsed: {}ms",
                now.elapsed().as_millis()
            );

            self.clients.insert(language_id.clone(), client.clone());

            self.capabilities
                .insert(language_id.clone(), initialize_result.capabilities);
        }

        Ok(())
    }

    async fn on_cursor_moved(&self, bufnr: usize) -> VimResult<()> {
        Ok(())
    }

    async fn goto_impl(&self, goto: Goto) -> Result<(), Error> {
        let bufnr = self.vim.bufnr("").await?;

        let filetype = self.vim.getbufvar::<String>(bufnr, "&filetype").await?;
        let client = self.clients.get(&filetype).ok_or(Error::ClientNotFound)?;

        let path = self.vim.bufabspath(bufnr).await?;
        let (_bufnr, row, column) = self.vim.get_cursor_pos().await?;
        let position = lsp::Position {
            line: row as u32 - 1,
            character: column as u32 - 1,
        };
        let text_document = lsp::TextDocumentIdentifier {
            uri: Url::from_file_path(&path).map_err(|_| Error::InvalidUrl(path))?,
        };

        tracing::debug!("starting goto");
        let now = std::time::Instant::now();
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

        if new_row != row && new_column != column {
            tracing::debug!("============= Cursor Pos changed! Cancel request");
        }

        match locations.len() {
            0 => {
                self.vim.echo_message("Definition not found")?;
            }
            1 => {
                self.vim.exec(
                    "clap#plugin#lsp#jump_to",
                    [FileLocation::from_lsp_location(&locations[0])],
                )?;
            }
            _ => {
                let file_locations = locations
                    .iter()
                    .map(FileLocation::from_lsp_location)
                    .collect::<Vec<_>>();
                self.vim.exec(
                    "clap#plugin#lsp#open_picker",
                    ("definitions", file_locations),
                )?;
            }
        }

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
            BufWritePost => {}
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
        }

        Ok(())
    }
}
