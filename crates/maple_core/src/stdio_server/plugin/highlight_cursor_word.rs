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
    // (line_number, highlight_col_start)
    other_words_highlight: Vec<(usize, usize)>,
    cword_highlight: (usize, usize),
    // highlight length.
    cword_len: usize,
}

/// Returns the char at given byte index in a line.
fn char_at(byte_idx: usize, line: &str) -> Option<char> {
    line.char_indices().enumerate().find_map(
        |(_c_idx, (b_idx, c))| {
            if byte_idx == b_idx {
                Some(c)
            } else {
                None
            }
        },
    )
}

fn find_word_highlights(
    source_file: &Path,
    line_start: usize,
    line_end: usize,
    curlnum: usize,
    col: usize,
    cword: String,
) -> std::io::Result<Option<WordHighlights>> {
    let cword_len = cword.len();
    let word_matcher = WordMatcher::new(vec![cword.into()]);
    // line_start and line_end is 1-based.
    let line_start = line_start - 1;
    let line_end = line_end - 1;
    let mut cursor_word_highlight = None;
    let other_words_highlight = read_lines_from(source_file, line_start, line_end - line_start)?
        .enumerate()
        .flat_map(|(idx, line)| {
            let matches_range = word_matcher.find_all_matches_range(&line);

            let line_number = idx + line_start + 1;

            if line_number == curlnum {
                let cursor_word_start = matches_range.iter().find_map(|word_range| {
                    if word_range.contains(&(col - 1)) {
                        Some(word_range.start)
                    } else {
                        None
                    }
                });
                if let Some(start) = cursor_word_start {
                    cursor_word_highlight.replace((line_number, start));
                }
            }

            matches_range.into_iter().filter_map(move |word_range| {
                // Skip the cursor word highlight.
                if line_number == curlnum && word_range.contains(&(col - 1)) {
                    None
                } else {
                    Some((line_number, word_range.start))
                }
            })
        })
        .collect();
    if let Some(cword_highlight) = cursor_word_highlight {
        Ok(Some(WordHighlights {
            other_words_highlight,
            cword_highlight,
            cword_len,
        }))
    } else {
        Ok(None)
    }
}

#[derive(Debug)]
struct WinHighlights {
    winid: usize,
    // matchaddpos() returns -1 on error.
    match_ids: Vec<i32>,
}

#[derive(Debug)]
pub struct CursorWordHighlighter {
    vim: Vim,
    cursor_highlights: Option<WinHighlights>,
}

impl CursorWordHighlighter {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            cursor_highlights: None,
        }
    }

    async fn create_new_highlights(&mut self) -> Result<Option<WinHighlights>> {
        let cword = self.vim.expand("<cword>").await?;

        // TODO: filter the false positive results, using a blocklist of filetypes?
        let curlnum = self.vim.line(".").await?;
        let col = self.vim.col(".").await?;
        let curline = self.vim.getcurbufline(curlnum).await?;

        let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '_';

        if let Some(cursor_char) = char_at(col-1, &curline) {
            if !is_word(cursor_char) {
                return Ok(None);
            }
        } else {
            return Ok(None);
        }

        if cword.is_empty() {
            return Ok(None);
        }

        let source_file = self.vim.current_buffer_path().await?;
        let source_file = Path::new(&source_file);

        if !source_file.is_file() {
            return Ok(None);
        }

        let winid = self.vim.current_winid().await?;

        // Lines in view.
        let line_start = self.vim.line("w0").await?;
        let line_end = self.vim.line("w$").await?;

        if let Ok(Some(word_highlights)) = find_word_highlights(
            source_file,
            line_start,
            line_end,
            curlnum,
            col,
            cword.clone(),
        ) {
            let match_ids: Vec<i32> = self
                .vim
                .call(
                    "clap#plugin#highlight_cursor_word#add_highlights",
                    word_highlights,
                )
                .await?;
            let new_highlights = WinHighlights { match_ids, winid };
            return Ok(Some(new_highlights));
        }

        Ok(None)
    }

    /// Highlight the cursor word and all the occurrences.
    async fn highlight_symbol_under_cursor(&mut self) -> Result<()> {
        let maybe_new_highlights = self.create_new_highlights().await?;
        let old_highlights = match maybe_new_highlights {
            Some(new_highlights) => self.cursor_highlights.replace(new_highlights),
            None => self.cursor_highlights.take(),
        };

        // Clear the old highlights after the new added ones so that no flicker occurs.
        if let Some(WinHighlights { winid, match_ids }) = old_highlights {
            self.vim.matchdelete_batch(match_ids, winid).await?;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapPlugin for CursorWordHighlighter {
    async fn on_autocmd(&mut self, autocmd: Autocmd) -> Result<()> {
        match autocmd {
            Autocmd::CursorMoved => self.highlight_symbol_under_cursor().await,
            Autocmd::InsertEnter => {
                if let Some(WinHighlights { winid, match_ids }) = self.cursor_highlights.take() {
                    self.vim.matchdelete_batch(match_ids, winid).await?;
                }
                Ok(())
            }
        }
    }
}
