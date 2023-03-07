use crate::stdio_server::input::Autocmd;
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use std::fmt::Debug;

/// A trait each Clap plugin must implement.
#[async_trait::async_trait]
pub trait ClapPlugin: Debug + Send + Sync + 'static {
    async fn on_autocmd(&mut self, autocmd: Autocmd) -> Result<()>;
}

#[derive(Debug)]
pub struct CursorWordHighligher {
    vim: Vim,
    // matchaddpos() returns -1 on error.
    current_highlights: Option<Vec<i32>>,
    last_cword: String,
}

#[derive(serde::Serialize)]
struct WordHighlights {
    highlights: Vec<(usize, usize)>,
    cword_len: usize,
}

impl CursorWordHighligher {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            current_highlights: None,
            last_cword: Default::default(),
        }
    }

    async fn highlight_cursor_word(&mut self) -> Result<()> {
        let cword = self.vim.expand("<cword>").await?;
        // TODO: filter the false positive results
        if cword.is_empty() {
            return Ok(());
        }

        if self.last_cword == cword {
            return Ok(());
        }

        if let Some(highlights) = self.current_highlights.take() {
            // clear the existing highlights
            for id in highlights {
                self.vim.matchdelete(id).await?;
            }
        }

        let source_file = self.vim.current_buffer_path().await?;
        let source_file = std::path::PathBuf::from(source_file);

        if !source_file.is_file() {
            return Ok(());
        }

        let start = self.vim.line("w0").await?;
        let end = self.vim.line("w$").await?;
        if let Ok(highlights) =
            crate::highlight_cursor_word::find_highlights(&source_file, start, end, cword.clone())
        {
            let cword_len = cword.len();
            self.last_cword = cword;
            let match_ids: Vec<i32> = self
                .vim
                .call(
                    "clap#highlight#add_cursor_word_highlight",
                    WordHighlights {
                        highlights,
                        cword_len,
                    },
                )
                .await?;
            self.current_highlights.replace(match_ids);
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapPlugin for CursorWordHighligher {
    async fn on_autocmd(&mut self, autocmd: Autocmd) -> Result<()> {
        match autocmd {
            Autocmd::CursorMoved => self.highlight_cursor_word().await,
        }
    }
}
