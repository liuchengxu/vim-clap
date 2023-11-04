use crate::stdio_server::input::{ActionRequest, AutocmdEvent, AutocmdEventType};
use crate::stdio_server::plugin::ClapPlugin;
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use matcher::WordMatcher;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use utils::read_lines_from;

#[derive(Debug, serde::Serialize)]
struct WordHighlights {
    // (line_number, highlight_col_start)
    twins_words_highlight: Vec<(usize, usize)>,
    cword_highlight: (usize, usize),
    // highlight length, in bytes.
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
    let twins_words_highlight = read_lines_from(source_file, line_start, line_end - line_start)?
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
            twins_words_highlight,
            cword_highlight,
            cword_len,
        }))
    } else {
        Ok(None)
    }
}

#[derive(Debug)]
struct CursorHighlights {
    winid: usize,
    // Use `i32` as matchaddpos() returns -1 on error.
    match_ids: Vec<i32>,
}

#[derive(Debug, maple_derive::ClapPlugin)]
#[clap_plugin(id = "cursorword")]
pub struct CursorWordPlugin {
    vim: Vim,
    bufs: HashMap<usize, PathBuf>,
    cursor_highlights: Option<CursorHighlights>,
    ignore_extensions: Vec<&'static str>,
    ignore_file_names: Vec<&'static str>,
}

impl CursorWordPlugin {
    pub fn new(vim: Vim) -> Self {
        let (ignore_extensions, ignore_file_names): (Vec<_>, Vec<_>) = crate::config::config()
            .plugin
            .cursorword
            .ignore_files
            .split(',')
            .partition(|s| s.starts_with("*."));

        Self {
            vim,
            bufs: HashMap::new(),
            cursor_highlights: None,
            ignore_extensions,
            ignore_file_names,
        }
    }

    async fn create_new_highlights(&mut self, bufnr: usize) -> Result<Option<CursorHighlights>> {
        let cword = self.vim.expand("<cword>").await?;

        if cword.is_empty() {
            return Ok(None);
        }

        let source_file = self
            .bufs
            .get(&bufnr)
            .ok_or_else(|| anyhow::anyhow!("bufnr doesn't exist"))?;

        // TODO: filter the false positive results, using a blocklist of filetypes?
        let [_bufnum, curlnum, col, _off] = self.vim.getpos(".").await?;
        let curline = self.vim.getbufoneline(bufnr, curlnum).await?;

        if crate::config::config()
            .plugin
            .cursorword
            .ignore_comment_line
        {
            if let Some(ext) = source_file.extension().and_then(|s| s.to_str()) {
                if dumb_analyzer::is_comment(curline.as_str(), ext) {
                    return Ok(None);
                }
            }
        }

        let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';

        if let Some(cursor_char) = char_at(col - 1, &curline) {
            if !is_word(cursor_char) {
                return Ok(None);
            }
        } else {
            return Ok(None);
        }

        let winid = self.vim.current_winid().await?;

        // Lines in view.
        let line_start = self.vim.line("w0").await?;
        let line_end = self.vim.line("w$").await?;

        if let Ok(Some(word_highlights)) =
            find_word_highlights(source_file, line_start, line_end, curlnum, col, cword)
        {
            let match_ids: Vec<i32> = self
                .vim
                .call("clap#plugin#cursorword#add_highlights", word_highlights)
                .await?;
            return Ok(Some(CursorHighlights { match_ids, winid }));
        }

        Ok(None)
    }

    /// Highlight the cursor word and all the occurrences.
    async fn highlight_symbol_under_cursor(&mut self, bufnr: usize) -> Result<()> {
        let maybe_new_highlights = self.create_new_highlights(bufnr).await?;
        let old_highlights = match maybe_new_highlights {
            Some(new_highlights) => self.cursor_highlights.replace(new_highlights),
            None => self.cursor_highlights.take(),
        };

        // Clear the old highlights after the new added ones so that no flicker occurs.
        if let Some(CursorHighlights { winid, match_ids }) = old_highlights {
            self.vim.matchdelete_batch(match_ids, winid).await?;
        }

        Ok(())
    }

    async fn try_track_buffer(&mut self, bufnr: usize) -> Result<()> {
        if self.bufs.contains_key(&bufnr) {
            return Ok(());
        }

        let source_file = self.vim.current_buffer_path().await?;
        let source_file = PathBuf::from(source_file);

        if !source_file.is_file() {
            return Ok(());
        }

        let Some(file_extension) = source_file.extension().and_then(|s| s.to_str()) else {
            return Ok(());
        };

        let Some(file_name) = source_file.file_name().and_then(|s| s.to_str()) else {
            return Ok(());
        };

        if self
            .ignore_extensions
            .iter()
            .any(|s| &s[2..] == file_extension)
            || self.ignore_file_names.contains(&file_name)
        {
            return Ok(());
        }

        self.bufs.insert(bufnr, source_file);

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapPlugin for CursorWordPlugin {
    async fn handle_action(&mut self, _action: ActionRequest) -> Result<()> {
        Ok(())
    }

    async fn handle_autocmd(&mut self, autocmd: AutocmdEvent) -> Result<()> {
        use AutocmdEventType::{
            BufDelete, BufEnter, BufLeave, BufWinEnter, BufWinLeave, CursorMoved, InsertEnter,
        };

        let (event_type, params) = autocmd;
        let bufnr = params.parse_bufnr()?;

        match event_type {
            BufEnter | BufWinEnter => self.try_track_buffer(bufnr).await?,
            BufDelete | BufLeave | BufWinLeave => {
                self.bufs.remove(&bufnr);
            }
            CursorMoved if self.bufs.contains_key(&bufnr) => {
                self.highlight_symbol_under_cursor(bufnr).await?
            }
            InsertEnter if self.bufs.contains_key(&bufnr) => {
                if let Some(CursorHighlights { winid, match_ids }) = self.cursor_highlights.take() {
                    self.vim.matchdelete_batch(match_ids, winid).await?;
                }
            }
            _ => {}
        }

        Ok(())
    }
}
