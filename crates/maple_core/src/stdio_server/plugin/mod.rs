mod ctags;
mod cursor_word_highlighter;
mod git;
mod markdown;

use crate::stdio_server::input::{PluginAction, PluginEvent};
use crate::stdio_server::vim::Vim;
use anyhow::{anyhow, Result};
use std::fmt::Debug;

pub use self::ctags::CtagsPlugin;
pub use self::cursor_word_highlighter::CursorWordHighlighter;
pub use self::git::GitPlugin;
pub use self::markdown::MarkdownPlugin;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum PluginId {
    System,
    Ctags,
    CursorWordHighlighter,
    Git,
    Markdown,
}

#[derive(Debug, Clone)]
pub struct Action {
    pub ty: ActionType,
    pub method: &'static str,
}

impl Action {
    pub const fn callable(method: &'static str) -> Self {
        Self {
            ty: ActionType::Callable,
            method,
        }
    }
}

#[derive(Debug, Clone)]
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

    fn actions(&self, _action_type: ActionType) -> &[Action] {
        &[]
    }

    async fn on_plugin_event(&mut self, plugin_event: PluginEvent) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct SystemPlugin {
    vim: Vim,
}

impl SystemPlugin {
    pub const ID: PluginId = PluginId::System;

    const NOTE_RECENT_FILES: &'static str = "note_recent_files";
    const NOTE_RECENT_FILES_ACTION: Action = Action::callable(Self::NOTE_RECENT_FILES);

    const OPEN_CONFIG: &'static str = "open-config";
    const OPEN_CONFIG_ACTION: Action = Action::callable(Self::OPEN_CONFIG);

    const CALLABLE_ACTIONS: &[Action] = &[Self::OPEN_CONFIG_ACTION];
    const ACTIONS: &[Action] = &[Self::NOTE_RECENT_FILES_ACTION, Self::OPEN_CONFIG_ACTION];

    pub fn new(vim: Vim) -> Self {
        Self { vim }
    }
}

#[async_trait::async_trait]
impl ClapPlugin for SystemPlugin {
    fn id(&self) -> PluginId {
        Self::ID
    }

    fn actions(&self, action_type: ActionType) -> &[Action] {
        match action_type {
            ActionType::Callable => Self::CALLABLE_ACTIONS,
            ActionType::All => Self::ACTIONS,
        }
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
                    _ => Ok(()),
                }
            }
        }
    }
}
