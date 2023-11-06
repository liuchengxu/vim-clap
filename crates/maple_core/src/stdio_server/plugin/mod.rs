mod colorizer;
mod ctags;
mod cursorword;
mod git;
mod linter;
mod markdown;
pub mod syntax;
mod system;

use crate::stdio_server::input::{ActionRequest, AutocmdEvent};
use anyhow::Result;
use std::fmt::Debug;

pub use self::colorizer::ColorizerPlugin;
pub use self::ctags::CtagsPlugin;
pub use self::cursorword::Cursorword as CursorwordPlugin;
pub use self::git::Git as GitPlugin;
pub use self::linter::Linter as LinterPlugin;
pub use self::markdown::Markdown as MarkdownPlugin;
pub use self::syntax::Syntax as SyntaxHighlighterPlugin;
pub use self::system::System as SystemPlugin;
pub use types::{Action, ActionType, ClapAction};

use super::input::AutocmdEventType;

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

    /// Returns the list of subscribed Autocmd events.
    fn subscriptions(&self) -> &[AutocmdEventType] {
        &[]
    }

    async fn handle_autocmd(&mut self, _autocmd: AutocmdEvent) -> Result<()> {
        Ok(())
    }
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
