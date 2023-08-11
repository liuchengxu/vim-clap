use crate::stdio_server::input::{AutocmdEventType, PluginEvent};
use crate::stdio_server::plugin::ClapPlugin;
use crate::stdio_server::vim::Vim;
use crate::tools::ctags::{BufferTag, Scope};
use anyhow::Result;
use icon::IconType;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

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

#[derive(Debug)]
pub struct CtagsPlugin {
    vim: Vim,
    last_cursor_tag: Option<BufferTag>,
    buf_tags: HashMap<usize, Vec<BufferTag>>,
}

impl CtagsPlugin {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            last_cursor_tag: None,
            buf_tags: HashMap::new(),
        }
    }

    async fn on_cursor_moved(&mut self, bufnr: usize) -> Result<()> {
        if let Some(buffer_tags) = self.buf_tags.get(&bufnr) {
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

                // Redraw the statusline to reflect the latest tag.
                self.vim.exec("execute", ["redrawstatus"])?;

                self.last_cursor_tag.replace(tag.clone());
            } else {
                self.vim.setbufvar(bufnr, "clap_current_symbol", {})?;
                self.last_cursor_tag.take();
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapPlugin for CtagsPlugin {
    async fn on_plugin_event(&mut self, plugin_event: PluginEvent) -> Result<()> {
        use AutocmdEventType::{BufDelete, BufEnter, BufWritePost, CursorMoved};

        let PluginEvent::Autocmd(autocmd_event) = plugin_event else {
          return Ok(());
        };

        let (event_type, params) = autocmd_event;

        let params: Vec<usize> = params.parse()?;
        let bufnr = params
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("bufnr not found in params"))?;

        match event_type {
            BufEnter | BufWritePost => {
                let file_path: String = self.vim.expand(format!("#{bufnr}:p")).await?;
                if !Path::new(&file_path).exists() {
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
            _ => {}
        }

        Ok(())
    }
}
