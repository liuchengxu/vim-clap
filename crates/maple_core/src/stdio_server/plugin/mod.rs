mod colorizer;
mod ctags;
mod cursorword;
mod git;
mod linter;
mod markdown;
pub mod syntax_highlighter;
mod system;

use crate::stdio_server::input::{ActionRequest, AutocmdEvent};
use anyhow::Result;
use std::fmt::Debug;

pub use self::colorizer::ColorizerPlugin;
pub use self::ctags::CtagsPlugin;
pub use self::cursorword::CursorWordPlugin;
pub use self::git::GitPlugin;
pub use self::linter::LinterPlugin;
pub use self::markdown::MarkdownPlugin;
pub use self::syntax_highlighter::SyntaxHighlighterPlugin;
pub use self::system::System as SystemPlugin;
pub use types::{Action, ActionType, ClapAction};

pub type PluginId = &'static str;

#[derive(Debug, Clone)]
pub enum Toggle {
    /// Plugin is enabled.
    On,
    /// Plugin is disabled.
    Off,
}

impl Toggle {
    pub fn switch(&mut self) {
        match self {
            Self::On => {
                *self = Self::Off;
            }
            Self::Off => {
                *self = Self::On;
            }
        }
    }

    pub fn is_off(&self) -> bool {
        matches!(self, Self::Off)
    }
}

/// A trait each Clap plugin must implement.
#[async_trait::async_trait]
pub trait ClapPlugin: ClapAction + Debug + Send + Sync + 'static {
    async fn handle_action(&mut self, action: ActionRequest) -> Result<()>;
    async fn handle_autocmd(&mut self, autocmd: AutocmdEvent) -> Result<()>;
}

#[cfg(test)]
mod tests {
    #[derive(maple_derive::ClapPlugin)]
    #[clap_plugin(id = "plugin", actions = ["action1", "action2"])]
    struct TestPlugin;

    #[derive(maple_derive::ClapPlugin)]
    #[clap_plugin(id = "empty")]
    struct EmptyPlugin;

    #[test]
    fn test_clap_plugin_attribute() {}
}
