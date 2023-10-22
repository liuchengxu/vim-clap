use crate::stdio_server::input::{PluginAction, PluginEvent};
use crate::stdio_server::plugin::{Action, ActionType, ClapAction, ClapPlugin, PluginId};
use crate::stdio_server::vim::Vim;
use anyhow::{anyhow, Result};

#[derive(Debug, Clone)]
pub struct SystemPlugin {
    vim: Vim,
}

impl SystemPlugin {
    const NOTE_RECENT_FILES: &'static str = "note_recent_files";
    const NOTE_RECENT_FILES_ACTION: Action = Action::callable(Self::NOTE_RECENT_FILES);

    const OPEN_CONFIG: &'static str = "open-config";
    const OPEN_CONFIG_ACTION: Action = Action::callable(Self::OPEN_CONFIG);

    const LIST_PLUGINS: &'static str = "list-plugins";
    const LIST_PLUGINS_ACTION: Action = Action::callable(Self::LIST_PLUGINS);

    const CALLABLE_ACTIONS: &[Action] = &[Self::OPEN_CONFIG_ACTION, Self::LIST_PLUGINS_ACTION];
    const ACTIONS: &[Action] = &[
        Self::NOTE_RECENT_FILES_ACTION,
        Self::OPEN_CONFIG_ACTION,
        Self::LIST_PLUGINS_ACTION,
    ];
}

impl ClapAction for SystemPlugin {
    fn actions(&self, action_type: ActionType) -> &[Action] {
        match action_type {
            ActionType::Callable => Self::CALLABLE_ACTIONS,
            ActionType::All => Self::ACTIONS,
        }
    }
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
