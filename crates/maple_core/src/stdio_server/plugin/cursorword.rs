use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType, PluginAction};
use crate::stdio_server::plugin::{ClapPlugin, PluginError};
use crate::stdio_server::vim::{Vim, VimError};
use colors_transform::Color;
use matcher::WordMatcher;
use rgb2ansi256::rgb_to_ansi256;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;
use utils::read_lines_from;
use AutocmdEventType::{
    BufDelete, BufEnter, BufLeave, BufWinEnter, BufWinLeave, CursorMoved, InsertEnter,
};

#[derive(Debug, serde::Serialize)]
struct WordHighlights {
    // (line_number, highlight_col_start)
    twins_words_highlight: Vec<(usize, usize)>,
    cword_highlight: (usize, usize),
    // highlight length, in bytes.
    cword_len: usize,
}

/// `line_start` and `curlnum` is 1-based line number.
fn find_word_highlights(
    lines: impl Iterator<Item = String>,
    line_start: usize,
    curlnum: usize,
    col: usize,
    cword: String,
) -> std::io::Result<Option<WordHighlights>> {
    let cword_len = cword.len();
    let word_matcher = WordMatcher::new(vec![cword.into()]);

    let mut cursor_word_highlight = None;
    let twins_words_highlight = lines
        .enumerate()
        .flat_map(|(index, line)| {
            let matches_range = word_matcher.find_all_matches_range(&line);

            let line_number = index + line_start;

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

async fn define_highlights(vim: &Vim) -> Result<(), PluginError> {
    let output = vim.call::<String>("execute", ["hi Normal"]).await?;
    let maybe_guibg = output.split('\n').find_map(|line| {
        line.split_whitespace()
            .find_map(|i| i.strip_prefix("guibg="))
    });
    if let Some(guibg) = maybe_guibg {
        let Ok(color) = colors_transform::Rgb::from_hex_str(guibg) else {
            return Ok(());
        };

        let light_color = color.lighten(10.0);
        let guibg = light_color.to_css_hex_string();
        let (r, g, b) = light_color.as_tuple();
        let ctermbg = rgb_to_ansi256(r as u8, g as u8, b as u8);

        let dark_color = color
            .lighten(-10.0)
            .adjust_color(colors_transform::RgbUnit::Red, 10.0);
        let twins_guibg = dark_color.to_css_hex_string();
        let (r, g, b) = dark_color.as_tuple();
        let twins_ctermbg = rgb_to_ansi256(r as u8, g as u8, b as u8);

        vim.exec(
            "clap#plugin#cursorword#define_highlights",
            [(ctermbg, guibg), (twins_ctermbg, twins_guibg)],
        )?;
    }
    Ok(())
}

#[derive(Debug, maple_derive::ClapPlugin)]
#[clap_plugin(id = "cursorword", actions = ["__defineHighlights"])]
pub struct Cursorword {
    vim: Vim,
    bufs: HashMap<usize, PathBuf>,
    cursor_highlights: Option<CursorHighlights>,
    ignore_extensions: Vec<&'static str>,
    ignore_file_names: Vec<&'static str>,
}

impl Cursorword {
    pub fn new(vim: Vim) -> Self {
        let (ignore_extensions, ignore_file_names): (Vec<_>, Vec<_>) = maple_config::config()
            .plugin
            .cursorword
            .ignore_files
            .split(',')
            .partition(|s| s.starts_with("*."));

        tokio::spawn({
            let vim = vim.clone();

            async move {
                if let Err(err) = define_highlights(&vim).await {
                    tracing::error!(?err, "[cursorword] Failed to define highlights");
                }
            }
        });

        Self {
            vim,
            bufs: HashMap::new(),
            cursor_highlights: None,
            ignore_extensions,
            ignore_file_names,
        }
    }

    async fn create_new_highlights(
        &mut self,
        bufnr: usize,
    ) -> Result<Option<CursorHighlights>, PluginError> {
        let cword = self.vim.expand("<cword>").await?;

        if cword.is_empty() {
            return Ok(None);
        }

        let source_file = self
            .bufs
            .get(&bufnr)
            .ok_or_else(|| VimError::InvalidBuffer)?;

        // TODO: filter the false positive results, using a blocklist of filetypes?
        let [_bufnum, curlnum, col, _off] = self.vim.getpos(".").await?;
        let curline = self.vim.getbufoneline(bufnr, curlnum).await?;

        if maple_config::config().plugin.cursorword.ignore_comment_line {
            if let Some(ext) = source_file.extension().and_then(|s| s.to_str()) {
                if code_tools::language::is_comment(curline.as_str(), ext) {
                    return Ok(None);
                }
            }
        }

        let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';

        if let Some(cursor_char) = utils::char_at(&curline, col - 1) {
            if !is_word(cursor_char) {
                return Ok(None);
            }
        } else {
            return Ok(None);
        }

        // Lines in view.
        let (winid, line_start, line_end) = self.vim.get_screen_lines_range().await?;

        let maybe_new_highlights = if self.vim.bufmodified(bufnr).await? {
            let lines = self.vim.getbufline(bufnr, line_start, line_end).await?;
            find_word_highlights(lines.into_iter(), line_start, curlnum, col, cword)
        } else {
            let lines = read_lines_from(source_file, line_start - 1, line_end - line_start + 1)?;
            find_word_highlights(lines, line_start, curlnum, col, cword)
        };

        if let Ok(Some(word_highlights)) = maybe_new_highlights {
            let match_ids: Vec<i32> = self
                .vim
                .call("clap#plugin#cursorword#add_highlights", word_highlights)
                .await?;
            return Ok(Some(CursorHighlights { match_ids, winid }));
        }

        Ok(None)
    }

    /// Highlight the cursor word and all the occurrences.
    async fn highlight_symbol_under_cursor(&mut self, bufnr: usize) -> Result<(), PluginError> {
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

    async fn clear_highlights(&mut self) -> Result<(), PluginError> {
        if let Some(CursorHighlights { winid, match_ids }) = self.cursor_highlights.take() {
            self.vim.matchdelete_batch(match_ids, winid).await?;
        }
        Ok(())
    }

    async fn try_track_buffer(&mut self, bufnr: usize) -> Result<(), PluginError> {
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
impl ClapPlugin for Cursorword {
    async fn handle_action(&mut self, action: PluginAction) -> Result<(), PluginError> {
        match self.parse_action(&action.method)? {
            CursorwordAction::__DefineHighlights => {
                define_highlights(&self.vim).await?;
            }
        }

        Ok(())
    }

    #[maple_derive::subscriptions]
    async fn handle_autocmd(&mut self, autocmd: AutocmdEvent) -> Result<(), PluginError> {
        let (event_type, params) = autocmd;
        let bufnr = params.parse_bufnr()?;

        match event_type {
            BufEnter | BufWinEnter => self.try_track_buffer(bufnr).await?,
            BufDelete | BufLeave | BufWinLeave => {
                self.bufs.remove(&bufnr);
                self.clear_highlights().await?;
            }
            CursorMoved => {
                if self.bufs.contains_key(&bufnr) {
                    self.highlight_symbol_under_cursor(bufnr).await?
                }
            }
            InsertEnter => {
                if self.bufs.contains_key(&bufnr) {
                    self.clear_highlights().await?;
                }
            }
            event => return Err(PluginError::UnhandledEvent(event)),
        }

        Ok(())
    }
}
