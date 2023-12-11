use crate::lsp::LanguageServerMessageHandler;
use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::plugin::{ActionRequest, ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::{Vim, VimResult};
use lsp::types::ServerCapabilities;
use lsp::Client;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

type BufferInfo = Vec<u8>;
type LanguageId = String;

fn find_project_root<'a>(filetype: &str, path: &'a Path) -> Option<&'a Path> {
    let root_markers = match filetype {
        "rust" => &["Cargo.toml"],
        _ => return None,
    };

    paths::find_project_root(path, root_markers)
}

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(
  id = "lsp",
  actions = [
    "debug",
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

    async fn on_buf_enter(&mut self, bufnr: usize) -> VimResult<()> {
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

            let enable_snippets = false;
            let initialize_result = client.initialize(enable_snippets).await?;

            client.notify::<lsp::types::notification::Initialized>(
                lsp::types::InitializedParams {},
            )?;

            self.clients.insert(language_id.clone(), client.clone());

            self.capabilities
                .insert(language_id.clone(), initialize_result.capabilities);
        }

        Ok(())
    }

    async fn on_cursor_moved(&self, bufnr: usize) -> VimResult<()> {
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
        }

        Ok(())
    }
}
