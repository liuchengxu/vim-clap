use crate::stdio_server::input::{AutocmdEvent, PluginAction};
use crate::stdio_server::plugin::ClapPlugin;
use crate::stdio_server::vim::Vim;
use anyhow::{anyhow, Result};

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(id = "system", actions = ["__note_recent_files", "open-config", "list-plugins"])]
pub struct System {
    vim: Vim,
}

impl System {
    pub fn new(vim: Vim) -> Self {
        Self { vim }
    }
}

#[async_trait::async_trait]
impl ClapPlugin for System {
    async fn handle_autocmd(&mut self, _autocmd: AutocmdEvent) -> Result<()> {
        Ok(())
    }

    async fn handle_action(&mut self, action: PluginAction) -> Result<()> {
        let PluginAction { method, params } = action;
        match method.as_str() {
            Self::__NOTE_RECENT_FILES => {
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
