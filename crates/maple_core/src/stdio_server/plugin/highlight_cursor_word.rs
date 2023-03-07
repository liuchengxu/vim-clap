use crate::stdio_server::input::Autocmd;
use crate::stdio_server::plugin::ClapPlugin;
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use matcher::WordMatcher;
use std::fmt::Debug;
use std::path::Path;
use utils::read_lines_from;

#[derive(Debug, serde::Serialize)]
struct WordHighlights {
    // (line_number, highlight_start)
    highlights: Vec<(usize, usize)>,
    // highlight length.
    cword_len: usize,
}

fn find_highlight_positions(
    source_file: &Path,
    line_start: usize,
    line_end: usize,
    cword: String,
) -> std::io::Result<WordHighlights> {
    let cword_len = cword.len();
    let word_matcher = WordMatcher::new(vec![cword.into()]);
    // line_start and line_end is 1-based.
    let start = line_start - 1;
    let end = line_end - 1;
    let highlights = read_lines_from(source_file, start, end - start)?
        .enumerate()
        .filter_map(|(idx, line)| {
            word_matcher
                .find_all_matches_in_byte_indices(&line)
                .and_then(|indices| indices.get(0).copied())
                .map(|highlight_start| (idx + start + 1, highlight_start))
        })
        .collect();
    Ok(WordHighlights {
        highlights,
        cword_len,
    })
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
        let lnum = self.vim.line(".").await?;
        let col = self.vim.col(".").await?;
        let curline = self.vim.getcurbufline(lnum).await?;

        if let Some(cursor_char) = curline.chars().nth(col - 1) {
            if cursor_char.is_whitespace()
                || cursor_char.is_ascii_punctuation()
                || cursor_char == '='
            {
                self.last_cword = cursor_char.to_string();
                return Ok(());
            }
        } else {
            return Ok(());
        }

        if cword.is_empty() {
            self.last_cword = cword;
            return Ok(());
        }

        let source_file = self.vim.current_buffer_path().await?;
        let source_file = Path::new(&source_file);

        if !source_file.is_file() {
            return Ok(());
        }

        let winid = self.vim.current_winid().await?;

        let line_start = self.vim.line("w0").await?;
        let line_end = self.vim.line("w$").await?;

        // TODO: Perhaps cache the lines in [start, end] as when the cursor moves, the lines may remain
        // unchanged.

        if let Ok(word_highlights) =
            find_highlight_positions(&source_file, line_start, line_end, cword.clone())
        {
            let match_ids: Vec<i32> = self
                .vim
                .call("clap#highlight#add_cursor_word_highlight", word_highlights)
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
