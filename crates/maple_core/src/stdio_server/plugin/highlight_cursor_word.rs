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
    other_words_highlight: Vec<(usize, usize)>,
    cword_highlight: (usize, usize),
    // highlight length.
    cword_len: usize,
}

fn find_highlight_positions(
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
    let mut current_word_highlight = None;
    let other_words_highlight = read_lines_from(source_file, line_start, line_end - line_start)?
        .enumerate()
        .flat_map(|(idx, line)| {
            let matches_range = word_matcher.find_all_matches_range(&line);

            let line_number = idx + line_start + 1;

            if line_number == curlnum {
                let cursor_word_start = matches_range.iter().find_map(|highlight_range| {
                    if highlight_range.contains(&(col - 1)) {
                        Some(highlight_range.start)
                    } else {
                        None
                    }
                });
                if let Some(start) = cursor_word_start {
                    current_word_highlight.replace((line_number, start));
                }
            }

            matches_range
                .into_iter()
                .filter_map(move |highlight_range| {
                    // Skip the cursor word highlight.
                    if line_number == curlnum && highlight_range.contains(&(col - 1)) {
                        None
                    } else {
                        Some((line_number, highlight_range.start))
                    }
                })
        })
        .collect();
    if let Some(cword_highlight) = current_word_highlight {
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
        let curlnum = self.vim.line(".").await?;
        let col = self.vim.col(".").await?;
        let curline = self.vim.getcurbufline(curlnum).await?;

        if let Some(cursor_char) = curline.chars().nth(col - 1) {
            if cursor_char.is_whitespace()
                || (cursor_char.is_ascii_punctuation() && cursor_char != '_')
                || cursor_char == '='
            {
                self.last_cword = cursor_char.to_string();
                return Ok(());
            }
        } else {
            self.last_cword = Default::default();
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

        if let Ok(Some(word_highlights)) = find_highlight_positions(
            source_file,
            line_start,
            line_end,
            curlnum,
            col,
            cword.clone(),
        ) {
            let match_ids: Vec<i32> = self
                .vim
                .call("clap#highlight#add_cursor_word_highlights", word_highlights)
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
