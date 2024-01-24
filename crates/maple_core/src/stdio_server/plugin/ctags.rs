use crate::stdio_server::input::{ActionRequest, AutocmdEvent, AutocmdEventType};
use crate::stdio_server::plugin::{ClapPlugin, PluginError};
use crate::stdio_server::vim::Vim;
use crate::tools::ctags::{BufferTag, Scope};
use icon::IconType;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use utils::SizeChecker;

#[derive(Serialize, Debug)]
struct ScopeRef<'a> {
    name: &'a str,
    scope_kind: &'a str,
    scope_kind_icon: IconType,
}

impl<'a> ScopeRef<'a> {
    fn from_scope(scope: &'a Scope) -> Self {
        let scope_kind_icon = icon::tags_kind_icon(&scope.scope_kind);
        Self {
            name: &scope.scope,
            scope_kind: &scope.scope_kind,
            scope_kind_icon,
        }
    }
}

#[derive(Debug, maple_derive::ClapPlugin)]
#[clap_plugin(id = "ctags")]
pub struct CtagsPlugin {
    vim: Vim,
    enable_winbar: Option<bool>,
    last_cursor_tag: Option<BufferTag>,
    buf_tags: HashMap<usize, Vec<BufferTag>>,
    file_size_checker: SizeChecker,
}

impl CtagsPlugin {
    pub fn new(vim: Vim) -> Self {
        let ctags_config = &crate::config::config().plugin.ctags;
        Self {
            vim,
            enable_winbar: if !ctags_config.enable_winbar {
                Some(false)
            } else {
                None
            },
            last_cursor_tag: None,
            buf_tags: HashMap::new(),
            file_size_checker: SizeChecker::new(ctags_config.max_file_size),
        }
    }

    async fn set_winbar(&self, bufnr: usize, tag: &BufferTag) -> Result<(), PluginError> {
        const SEP: char = '';

        let winid = self.vim.bare_call::<usize>("nvim_get_current_win").await?;

        if self.vim.call::<usize>("winbufnr", [winid]).await? == bufnr {
            let mut winbar_items = if let Some(scope) = &tag.scope {
                vec![
                    ("LineNr", format!("\\ {SEP}\\ ")),
                    (
                        "Include",
                        format!("{}", icon::tags_kind_icon(&scope.scope_kind)),
                    ),
                    ("Normal", format!("\\ {}", &scope.scope)),
                ]
            } else {
                Vec::new()
            };

            winbar_items.extend([
                ("LineNr", format!("\\ {SEP}\\ ")),
                ("Type", format!("{}", icon::tags_kind_icon(&tag.kind))),
                ("Normal", format!("\\ {}", &tag.name)),
            ]);

            self.vim
                .exec("clap#api#set_winbar", (winid, winbar_items))?;
        }

        Ok(())
    }

    /// Updates the buffer variable `clap_current_symbol`.
    async fn on_cursor_moved(&mut self, bufnr: usize) -> Result<(), PluginError> {
        let Some(buffer_tags) = self.buf_tags.get(&bufnr) else {
            return Ok(());
        };

        let curlnum = self.vim.line(".").await?;
        let idx = match buffer_tags.binary_search_by_key(&curlnum, |tag| tag.line_number) {
            Ok(idx) => idx,
            Err(idx) => match idx.checked_sub(1) {
                Some(idx) => idx,
                None => {
                    // Before the first tag.
                    self.vim.setbufvar(bufnr, "clap_current_symbol", {})?;
                    self.last_cursor_tag.take();
                    return Ok(());
                }
            },
        };
        if let Some(tag) = buffer_tags.get(idx) {
            if let Some(last_cursor_tag) = &self.last_cursor_tag {
                if last_cursor_tag == tag {
                    return Ok(());
                }
            }

            self.vim.setbufvar(
                bufnr,
                "clap_current_symbol",
                serde_json::json!({
                    "name": tag.name,
                    "line_number": tag.line_number,
                    "kind": tag.kind,
                    "kind_icon": icon::tags_kind_icon(&tag.kind),
                    "scope": tag.scope.as_ref().map(ScopeRef::from_scope),
                }),
            )?;

            let winbar_enabled = match self.enable_winbar {
                Some(x) => x,
                None => {
                    // Neovim-only
                    let is_nvim = self
                        .vim
                        .call::<usize>("has", "nvim")
                        .await
                        .map(|x| x == 1)
                        .unwrap_or(false);

                    self.enable_winbar.replace(is_nvim);

                    is_nvim
                }
            };

            if winbar_enabled {
                self.set_winbar(bufnr, &tag).await?;
            }

            // Redraw the statusline to reflect the latest tag.
            self.vim.exec("execute", ["redrawstatus"])?;

            self.last_cursor_tag.replace(tag.clone());
        } else {
            self.vim.setbufvar(bufnr, "clap_current_symbol", {})?;
            self.last_cursor_tag.take();
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapPlugin for CtagsPlugin {
    async fn handle_action(&mut self, _action: ActionRequest) -> Result<(), PluginError> {
        Ok(())
    }

    #[maple_derive::subscriptions]
    async fn handle_autocmd(&mut self, autocmd: AutocmdEvent) -> Result<(), PluginError> {
        use AutocmdEventType::{BufDelete, BufEnter, BufWritePost, CursorMoved};

        let (event_type, params) = autocmd;

        let bufnr = params.parse_bufnr()?;

        match event_type {
            BufEnter | BufWritePost => {
                let file_path: String = self.vim.expand(format!("#{bufnr}:p")).await?;
                if !Path::new(&file_path).exists()
                    || self.file_size_checker.is_too_large(&file_path)?
                {
                    return Ok(());
                }
                let buffer_tags = crate::tools::ctags::fetch_buffer_tags(file_path)?;
                self.buf_tags.insert(bufnr, buffer_tags);
                self.on_cursor_moved(bufnr).await?;
            }
            BufDelete => {
                self.buf_tags.remove(&bufnr);
            }
            CursorMoved => self.on_cursor_moved(bufnr).await?,
            event => return Err(PluginError::UnhandledEvent(event)),
        }

        Ok(())
    }
}
