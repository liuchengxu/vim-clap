mod ctags;
mod cursor_word_highlighter;
mod markdown;

use crate::stdio_server::input::{PluginAction, PluginEvent};
use crate::stdio_server::vim::Vim;
use anyhow::{anyhow, Result};
use std::fmt::Debug;

pub use ctags::CtagsPlugin;
pub use cursor_word_highlighter::CursorWordHighlighter;
pub use markdown::MarkdownPlugin;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum PluginId {
    System,
    Ctags,
    CursorWordHighlighter,
    Markdown,
}

#[derive(Debug)]
pub enum ActionType {
    /// Actions that users can interact with.
    Callable,
    /// All actions.
    All,
}

/// A trait each Clap plugin must implement.
#[async_trait::async_trait]
pub trait ClapPlugin: Debug + Send + Sync + 'static {
    fn id(&self) -> PluginId;

    fn actions(&self, _action_type: ActionType) -> &[&'static str] {
        &[]
    }

    async fn on_plugin_event(&mut self, plugin_event: PluginEvent) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct SystemPlugin {
    vim: Vim,
}

impl SystemPlugin {
    const NOTE_RECENT_FILES: &'static str = "note_recent_files";

    const OPEN_CONFIG: &'static str = "open-config";

    const CALLABLE_ACTIONS: &[&'static str] = &[Self::OPEN_CONFIG];

    pub const ID: PluginId = PluginId::System;
    pub const ACTIONS: &[&'static str] = &[Self::NOTE_RECENT_FILES, Self::OPEN_CONFIG];

    pub fn new(vim: Vim) -> Self {
        Self { vim }
    }
}

#[async_trait::async_trait]
impl ClapPlugin for SystemPlugin {
    fn id(&self) -> PluginId {
        Self::ID
    }

    fn actions(&self, action_type: ActionType) -> &[&'static str] {
        match action_type {
            ActionType::Callable => Self::CALLABLE_ACTIONS,
            ActionType::All => Self::ACTIONS,
        }
    }

    async fn on_plugin_event(&mut self, plugin_event: PluginEvent) -> Result<()> {
        match plugin_event {
            PluginEvent::Autocmd(_) => Ok(()),
            PluginEvent::Action(plugin_action) => {
                let PluginAction { action, params } = plugin_action;
                match action.as_str() {
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
                    _ => Ok(()),
                }
            }
        }
    }
}
