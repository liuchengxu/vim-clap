mod highlight_cursor_word;

use crate::stdio_server::input::Autocmd;
use anyhow::Result;
use std::fmt::Debug;

pub use highlight_cursor_word::CursorWordHighlighter;

/// A trait each Clap plugin must implement.
#[async_trait::async_trait]
pub trait ClapPlugin: Debug + Send + Sync + 'static {
    async fn on_autocmd(&mut self, autocmd: Autocmd) -> Result<()>;
}
