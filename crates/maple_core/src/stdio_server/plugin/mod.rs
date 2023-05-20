mod highlight_cursor_word;
mod markdown_toc;
mod syntax_highlighter;

use crate::stdio_server::input::PluginEvent;
use anyhow::Result;
use std::fmt::Debug;

pub use highlight_cursor_word::CursorWordHighlighter;
pub use markdown_toc::{find_toc_range, generate_toc};
pub use syntax_highlighter::SyntaxHighlighter;

/// A trait each Clap plugin must implement.
#[async_trait::async_trait]
pub trait ClapPlugin: Debug + Send + Sync + 'static {
    async fn handle_event(&mut self, event: PluginEvent) -> Result<()>;
}
