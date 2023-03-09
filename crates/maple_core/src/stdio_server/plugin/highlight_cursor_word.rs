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
struct WinHighlights {
    winid: usize,
    // matchaddpos() returns -1 on error.
    match_ids: Vec<i32>,
}

#[derive(Debug)]
pub struct CursorWordHighlighter {
    vim: Vim,
    current_highlights: Option<WinHighlights>,
    last_cword: String,
}

impl CursorWordHighlighter {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            current_highlights: None,
            last_cword: Default::default(),
        }
    }

    async fn create_new_highlights(&mut self) -> Result<Option<WinHighlights>> {
        let cword = self.vim.expand("<cword>").await?;

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
                return Ok(None);
            }
        } else {
            self.last_cword = Default::default();
            return Ok(None);
        }

        if cword.is_empty() {
            self.last_cword = cword;
            return Ok(None);
        }

        let source_file = self.vim.current_buffer_path().await?;
        let source_file = Path::new(&source_file);

        if !source_file.is_file() {
            return Ok(None);
        }

        let winid = self.vim.current_winid().await?;

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
            self.last_cword = cword;
            let new_highlights = WinHighlights { match_ids, winid };
            return Ok(Some(new_highlights));
        }

        Ok(None)
    }

    async fn highlight_cursor_word(&mut self) -> Result<()> {
        let maybe_new_highlights = self.create_new_highlights().await?;
        let old_highlights = match maybe_new_highlights {
            Some(new_highlights) => self.current_highlights.replace(new_highlights),
            None => self.current_highlights.take(),
        };

        if let Some(WinHighlights { winid, match_ids }) = old_highlights {
            // clear the existing highlights
            if self.vim.win_is_valid(winid).await? {
                for match_id in match_ids {
                    self.vim.matchdelete(match_id, winid).await?;
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapPlugin for CursorWordHighlighter {
    async fn on_autocmd(&mut self, autocmd: Autocmd) -> Result<()> {
        match autocmd {
            Autocmd::CursorMoved => self.highlight_cursor_word().await,
        }
    }
}
