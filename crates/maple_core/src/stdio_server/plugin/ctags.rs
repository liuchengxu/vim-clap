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
    scope: &'a str,
    scope_kind: &'a str,
    scope_kind_icon: IconType,
}

impl<'a> ScopeRef<'a> {
    fn from_scope(scope: &'a Scope) -> Self {
        let scope_kind_icon = icon::tags_kind_icon(&scope.scope_kind);
        Self {
            scope: &scope.scope,
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
}

#[async_trait::async_trait]
impl ClapPlugin for CtagsPlugin {
    async fn on_plugin_event(&mut self, plugin_event: PluginEvent) -> Result<()> {
        use AutocmdEventType::{BufDelete, BufEnter, CursorMoved};

        let PluginEvent::Autocmd(autocmd_event) = plugin_event;

        let (autocmd_event_type, params) = autocmd_event;

        let params: Vec<usize> = params.parse()?;
        let bufnr = params
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("bufnr not found in params"))?;

        match autocmd_event_type {
            BufEnter => {
                let file_path: String = self.vim.expand(format!("#{bufnr}:p")).await?;
                if !Path::new(&file_path).exists() {
                    return Ok(());
                }
                if !self.buf_tags.contains_key(&bufnr) {
                    let buffer_tags = crate::tools::ctags::fetch_buffer_tags(file_path)?;
                    self.buf_tags.insert(bufnr, buffer_tags);
                }
            }
            BufDelete => {
                self.buf_tags.remove(&bufnr);
            }
            CursorMoved => {
                let [_bufnum, curlnum, _col, _off] = self.vim.getpos(".").await?;
                if let Some(buffer_tags) = self.buf_tags.get(&bufnr) {
                    let idx =
                        match buffer_tags.binary_search_by_key(&curlnum, |tag| tag.line_number) {
                            Ok(idx) => idx,
                            Err(idx) => idx.saturating_sub(1),
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

                        self.last_cursor_tag.replace(tag.clone());
                    } else {
                        self.vim.setbufvar(bufnr, "clap_current_symbol", {})?;
                        self.last_cursor_tag.take();
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
}
