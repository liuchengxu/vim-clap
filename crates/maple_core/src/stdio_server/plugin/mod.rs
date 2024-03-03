mod colorizer;
mod ctags;
mod cursorword;
mod diagnostics;
mod git;
mod linter;
mod lsp;
mod markdown;
pub mod syntax;
mod system;

use self::lsp::Error as LspError;
use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType, PluginAction};
use crate::stdio_server::vim::VimError;
use std::fmt::Debug;

pub use self::colorizer::ColorizerPlugin;
pub use self::ctags::CtagsPlugin;
pub use self::cursorword::Cursorword as CursorwordPlugin;
pub use self::diagnostics::Diagnostics as DiagnosticsPlugin;
pub use self::git::Git as GitPlugin;
pub use self::linter::Linter as LinterPlugin;
pub use self::lsp::LspPlugin;
pub use self::markdown::Markdown as MarkdownPlugin;
pub use self::syntax::Syntax as SyntaxPlugin;
pub use self::system::System as SystemPlugin;
pub use types::{ActionType, ClapAction};

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

    pub fn turn_on(&mut self) {
        if self.is_off() {
            *self = Self::On;
        }
    }

    pub fn is_off(&self) -> bool {
        matches!(self, Self::Off)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("unhandled {0:?}, possibly a bug caused by incomplete subscriptions.")]
    UnhandledEvent(AutocmdEventType),
    #[error("bufnr not found in params of request `{0}`")]
    MissingBufferNumber(&'static str),
    #[error("{0}")]
    Other(String),
    #[error(transparent)]
    GitPlugin(#[from] crate::tools::git::GitError),
    #[error("tree sitter highlighting error: {0:?}")]
    Highlight(#[from] tree_sitter::HighlightError),
    #[error(transparent)]
    Vim(#[from] VimError),
    #[error(transparent)]
    Lsp(#[from] LspError),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    Path(#[from] std::path::StripPrefixError),
    #[error(transparent)]
    FromUtf8(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    Clipboard(#[from] Box<dyn std::error::Error + Send + Sync>),
}

pub type PluginResult<T> = std::result::Result<T, PluginError>;

/// A trait each Clap plugin must implement.
#[async_trait::async_trait]
pub trait ClapPlugin: ClapAction + Debug + Send + Sync + 'static {
    /// Returns the list of subscribed Autocmd events.
    ///
    /// The default implementation returns an empty list of subscriptions, which means the plugin
    /// is not listening to any autocmd events by default.
    ///
    /// If the provider handles any autocmd events, the corresponding autocmd subscriptions
    /// can be implemented automatically by applying the derive macro `#[maple_derive::subscriptions]`
    /// on the function [`Self::handle_autocmd`].
    ///
    /// # Examples
    ///
    /// ```no_compile
    /// #[maple_derive::subscriptions]
    /// async fn handle_autocmd(&mut self, _autocmd: AutocmdEvent) -> Result<(), PluginError> {
    ///     // Subscribe and handle certains autocmd events.
    ///     // ..
    ///     Ok(())
    /// }
    /// ```
    fn subscriptions(&self) -> &[AutocmdEventType] {
        &[]
    }

    async fn handle_autocmd(&mut self, _autocmd: AutocmdEvent) -> Result<(), PluginError> {
        Ok(())
    }

    async fn handle_action(&mut self, action: PluginAction) -> Result<(), PluginError>;

    async fn handle_config_reloaded(&mut self) -> Result<(), PluginError> {
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
