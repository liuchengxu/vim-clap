use crate::stdio_server::input::{PluginAction, PluginEvent};
use crate::stdio_server::plugin::{ClapPlugin, PluginId};
use crate::stdio_server::vim::Vim;
use anyhow::{anyhow, Result};

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[actions("__note_recent_files", "open-config", "list-plugins")]
pub struct SystemPlugin {
    vim: Vim,
}

impl SystemPlugin {
    pub const ID: PluginId = PluginId::System;

    pub fn new(vim: Vim) -> Self {
        Self { vim }
    }
}

#[async_trait::async_trait]
impl ClapPlugin for SystemPlugin {
    fn id(&self) -> PluginId {
        Self::ID
    }

    async fn on_plugin_event(&mut self, plugin_event: PluginEvent) -> Result<()> {
        match plugin_event {
            PluginEvent::Autocmd(_) => Ok(()),
            PluginEvent::Action(plugin_action) => {
                let PluginAction { method, params } = plugin_action;
                match method.as_str() {
                    Self::NOTE_RECENT_FILES => {
                        let bufnr: Vec<usize> = params.parse()?;
                        let bufnr = bufnr
                            .first()
                            .ok_or(anyhow!("bufnr not found in `note_recent_files`"))?;
                        let file_path: String = self.vim.expand(format!("#{bufnr}:p")).await?;
                        crate::stdio_server::handler::messages::note_recent_file(file_path)
                    }
                    Self::OPEN_CONFIG => {
                        let config_file = crate::config::config_file();
                        self.vim
                            .exec("execute", format!("edit {}", config_file.display()))
                    }
                    Self::LIST_PLUGINS => {
                        // Handled upper level.
                        Ok(())
                    }
                    _ => Ok(()),
                }
            }
        }
    }
}
