use crate::stdio_server::input::Autocmd;
use crate::stdio_server::plugin::ClapPlugin;
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use matcher::WordMatcher;
use std::fmt::Debug;
use std::path::Path;
use utils::read_lines_from;

fn find_highlight_positions(
    source_file: &Path,
    start: usize,
    end: usize,
    word: String,
) -> std::io::Result<Vec<(usize, usize)>> {
    let word_matcher = WordMatcher::new(vec![word.into()]);
    Ok(read_lines_from(source_file, start, end - start)?
        .enumerate()
        .filter_map(|(idx, line)| {
            word_matcher
                .find_matches(&line)
                .and_then(|(_score, indices)| indices.get(0).copied())
                .map(|highlight_start| (idx + start + 1, highlight_start))
        })
        .collect())
}

#[derive(Debug)]
pub struct CursorWordHighligher {
    vim: Vim,
    current_highlights: Option<CurrentHighlights>,
    last_cword: String,
}

#[derive(Debug)]
struct CurrentHighlights {
    // matchaddpos() returns -1 on error.
    match_ids: Vec<i32>,
    winid: usize,
}

#[derive(serde::Serialize)]
struct WordHighlights {
    // (line_number, highlight_start)
    highlights: Vec<(usize, usize)>,
    // highlight length.
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

        if self.last_cword == cword {
            return Ok(());
        }

        if let Some(CurrentHighlights { match_ids, winid }) = self.current_highlights.take() {
            // clear the existing highlights
            if self.vim.win_is_valid(winid).await? {
                for match_id in match_ids {
                    self.vim.matchdelete(match_id, winid).await?;
                }
            }
        }

        // TODO: filter the false positive results, using a blocklist of filetypes?
        if cword.is_empty() {
            return Ok(());
        }

        let source_file = self.vim.current_buffer_path().await?;
        let source_file = Path::new(&source_file);

        if !source_file.is_file() {
            return Ok(());
        }

        let winid = self.vim.current_winid().await?;
        let start = self.vim.line("w0").await?;
        let end = self.vim.line("w$").await?;

        // TODO: Perhaps cache the lines in [start, end] as when the cursor moves, the lines may remain
        // unchanged.

        if let Ok(highlights) = find_highlight_positions(&source_file, start, end, cword.clone()) {
            let cword_len = cword.len();
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
            self.last_cword = cword;
            self.current_highlights
                .replace(CurrentHighlights { match_ids, winid });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_cursor_word() {
        let res = find_highlight_positions(
            Path::new(
                "/home/xlc/.vim/plugged/vim-clap/crates/maple_core/src/highlight_cursor_word.rs",
            ),
            1,
            30,
            "line".to_string(),
        )
        .unwrap();
        println!("res: {res:?}");
    }
}
