mod ctags;
mod cursor_word_highlighter;
mod git;
mod markdown;
mod system;

use crate::stdio_server::input::{PluginAction, PluginEvent};
use anyhow::Result;
use std::fmt::Debug;

pub use self::ctags::CtagsPlugin;
pub use self::cursor_word_highlighter::CursorWordHighlighter;
pub use self::git::GitPlugin;
pub use self::markdown::MarkdownPlugin;
pub use self::system::SystemPlugin;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum PluginId {
    System,
    Ctags,
    CursorWordHighlighter,
    Git,
    Markdown,
}

impl std::fmt::Display for PluginId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::Ctags => write!(f, "ctags"),
            Self::CursorWordHighlighter => write!(f, "cursor-word-highlighter"),
            Self::Git => write!(f, "git"),
            Self::Markdown => write!(f, "markdown"),
        }
    }
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

pub trait ClapAction {
    fn actions(&self, _action_type: ActionType) -> &[Action] {
        &[]
    }
}

/// A trait each Clap plugin must implement.
#[async_trait::async_trait]
pub trait ClapPlugin: ClapAction + Debug + Send + Sync + 'static {
    fn id(&self) -> PluginId;

    async fn on_plugin_event(&mut self, plugin_event: PluginEvent) -> Result<()>;
}
