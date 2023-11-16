use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::path::Path;

use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::plugin::{ActionRequest, ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::Vim;
use itertools::Itertools;
use once_cell::sync::Lazy;
use sublime_syntax::{SyntaxReference, TokenHighlight};
use tree_sitter::Language;

pub static SUBLIME_SYNTAX_HIGHLIGHTER: Lazy<sublime_syntax::SyntaxHighlighter> =
    Lazy::new(sublime_syntax::SyntaxHighlighter::new);

#[allow(unused)]
#[derive(Debug)]
struct SyntaxProps {
    row: usize,
    range: Range<usize>,
    length: usize,
    node: &'static str,
}

#[derive(Debug, Clone)]
struct BufferHighlights(BTreeMap<usize, Vec<tree_sitter::HighlightItem>>);

impl BufferHighlights {
    fn syntax_props_at(
        &self,
        language: Language,
        row: usize,
        column: usize,
    ) -> Option<SyntaxProps> {
        self.0.get(&row).and_then(|highlights| {
            highlights.iter().find_map(|h| {
                if (h.start.column..h.end.column).contains(&column) {
                    Some(SyntaxProps {
                        row: h.start.row,
                        range: h.start.column..h.end.column,
                        length: h.end.column - h.start.column,
                        node: language.highlight_name(h.highlight),
                    })
                } else {
                    None
                }
            })
        })
    }
}

impl From<BTreeMap<usize, Vec<tree_sitter::HighlightItem>>> for BufferHighlights {
    fn from(inner: BTreeMap<usize, Vec<tree_sitter::HighlightItem>>) -> Self {
        Self(inner)
    }
}

/// (start, length, highlight_group)
type LineHighlights = Vec<(usize, usize, &'static str)>;
type VimHighlights = Vec<(usize, LineHighlights)>;

#[derive(Debug, Clone)]
struct TreeSitterInfo {
    language: Language,
    highlights: BufferHighlights,
    /// Current highlighting info.
    vim_highlights: VimHighlights,
}

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(id = "syntax", actions = [
    "list-sublime-themes",
    "sublime-syntax-highlight",
    "tree-sitter-highlight",
    "tree-sitter-list-scopes",
    "tree-sitter-props-at-cursor",
    "toggle",
])]
pub struct Syntax {
    vim: Vim,
    toggle: Toggle,
    ts_bufs: HashMap<usize, TreeSitterInfo>,
    sublime_bufs: HashMap<usize, String>,
    tree_sitter_enabled: bool,
    sublime_syntax_enabled: bool,
}

impl Syntax {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            toggle: Toggle::Off,
            ts_bufs: HashMap::new(),
            sublime_bufs: HashMap::new(),
            tree_sitter_enabled: false,
            sublime_syntax_enabled: false,
        }
    }

    async fn on_buf_enter(&mut self, bufnr: usize) -> Result<(), PluginError> {
        let fpath = self.vim.bufabspath(bufnr).await?;
        if let Some(extension) = std::path::Path::new(&fpath)
            .extension()
            .and_then(|e| e.to_str())
        {
            self.sublime_bufs.insert(bufnr, extension.to_string());

            if self.tree_sitter_enabled {
                if let Some(language) = tree_sitter::Language::try_from_extension(extension) {
                    self.tree_sitter_highlight(bufnr, false, Some(language))
                        .await?;
                    self.toggle.turn_on();
                } else {
                    let filetype = self.vim.getbufvar::<String>(bufnr, "&filetype").await?;

                    if let Some(language) = tree_sitter::Language::try_from_filetype(&filetype) {
                        self.tree_sitter_highlight(bufnr, false, Some(language))
                            .await?;
                        self.toggle.turn_on();
                    }
                }
            }
        }

        Ok(())
    }

    /// Highlight the visual lines of specified buffer.
    // TODO: this may be inaccurate, e.g., the highlighted lines are part of a bigger block of comments.
    async fn sublime_syntax_highlight(&mut self, bufnr: usize) -> Result<(), PluginError> {
        let Some(extension) = self.sublime_bufs.get(&bufnr) else {
            return Ok(());
        };

        let highlighter = &SUBLIME_SYNTAX_HIGHLIGHTER;
        let Some(syntax) = highlighter.syntax_set.find_syntax_by_extension(extension) else {
            tracing::debug!("Can not find syntax for extension {extension}");
            return Ok(());
        };

        let line_start = self.vim.line("w0").await?;
        let end = self.vim.line("w$").await?;
        let lines = self.vim.getbufline(bufnr, line_start, end).await?;

        // const THEME: &str = "Coldark-Dark";
        const THEME: &str = "Visual Studio Dark+";

        // TODO: This influences the Normal highlight of vim syntax theme that is different from
        // the sublime text syntax theme here.
        if let Some((guifg, ctermfg)) = highlighter.get_normal_highlight(THEME) {
            self.vim.exec(
                "execute",
                format!("hi! Normal guifg={guifg} ctermfg={ctermfg}"),
            )?;
        }

        let now = std::time::Instant::now();

        let line_highlights = sublime_syntax_highlight(syntax, lines.iter(), line_start, THEME);

        self.vim.exec(
            "clap#highlighter#highlight_lines",
            (bufnr, &line_highlights),
        )?;

        tracing::debug!("Lines highlight elapsed: {:?}ms", now.elapsed().as_millis());

        Ok(())
    }

    async fn identify_buffer_language(&self, bufnr: usize, source_file: &Path) -> Option<Language> {
        if let Some(language) = source_file.extension().and_then(|e| {
            e.to_str()
                .and_then(tree_sitter::Language::try_from_extension)
        }) {
            Some(language)
        } else if let Ok(filetype) = self.vim.getbufvar::<String>(bufnr, "&filetype").await {
            tree_sitter::Language::try_from_filetype(&filetype)
        } else {
            None
        }
    }

    async fn tree_sitter_highlight(
        &mut self,
        bufnr: usize,
        buf_modified: bool,
        maybe_language: Option<Language>,
    ) -> Result<(), PluginError> {
        let source_file = self.vim.bufabspath(bufnr).await?;
        let source_file = std::path::PathBuf::from(source_file);

        let language = match maybe_language {
            Some(language) => language,
            None => {
                let Some(language) = self.identify_buffer_language(bufnr, &source_file).await
                else {
                    // No language detected, fallback to the vim regex syntax highlighting.
                    self.vim.exec("execute", "syntax on")?;
                    return Ok(());
                };

                language
            }
        };

        let source_code = if buf_modified {
            // TODO: this request the entire buffer content, which might be performance sensitive
            // in case of large buffer, we should add some kind of buffer size limit.
            //
            // Optimization: Get changed lines and apply to the previous version on the disk?
            let lines = self.vim.getbufline(bufnr, 1, "$").await?;
            lines.join("\n").into_bytes()
        } else {
            std::fs::read(&source_file)?
        };

        if self.vim.eval::<usize>("exists('g:syntax_on')").await? != 0 {
            self.vim.exec("execute", "syntax off")?;
        }

        let buffer_highlights = tree_sitter::highlight(language, &source_code)?;

        let (_winid, line_start, line_end) = self.vim.get_screen_lines_range().await?;
        let maybe_vim_highlights = self.apply_ts_highlights(
            bufnr,
            language,
            &buffer_highlights,
            Some(line_start - 1..line_end),
        )?;

        self.ts_bufs.insert(
            bufnr,
            TreeSitterInfo {
                language,
                highlights: buffer_highlights.into(),
                vim_highlights: maybe_vim_highlights.unwrap_or_default(),
            },
        );

        Ok(())
    }

    fn apply_ts_highlights(
        &self,
        bufnr: usize,
        language: Language,
        buffer_highlights: &BTreeMap<usize, Vec<tree_sitter::HighlightItem>>,
        lines_range: Option<Range<usize>>,
    ) -> Result<Option<VimHighlights>, PluginError> {
        // Convert the raw highlight info to something that is easily applied by Vim.
        let new_vim_highlights = buffer_highlights
            .iter()
            .filter(|(line_number, _)| {
                lines_range
                    .as_ref()
                    .map(|range| range.contains(line_number))
                    .unwrap_or(true)
            })
            .map(|(line_number, highlight_items)| {
                let line_highlights: Vec<(usize, usize, &str)> = highlight_items
                    .iter()
                    .map(|i| {
                        (
                            i.start.column,
                            i.end.column - i.start.column,
                            language.highlight_group(i.highlight),
                        )
                    })
                    .collect();

                (*line_number, line_highlights)
            })
            .collect::<Vec<_>>();

        if let Some(old) = self.ts_bufs.get(&bufnr) {
            let old_vim_highlights = &old.vim_highlights;

            let (unchanged_highlights, changed_highlights): (Vec<_>, Vec<_>) = new_vim_highlights
                .iter()
                .partition(|item| old_vim_highlights.contains(item));

            let unchanged_highlights = unchanged_highlights
                .into_iter()
                .map(|item| item.0)
                .collect::<Vec<_>>();

            let mut changed_highlights = changed_highlights
                .into_iter()
                .map(|item| item.0)
                .collect::<Vec<_>>();

            tracing::debug!(
                total = new_vim_highlights.len(),
                unchanged = unchanged_highlights.len(),
                changed = changed_highlights.len(),
                "Applying new highlights",
            );

            // No new highlight changes since the last highlighting operation.
            if changed_highlights.is_empty() {
                return Ok(None);
            }

            // Keep the changed highlights only.
            let diff_highlights = new_vim_highlights
                .iter()
                .filter(|item| changed_highlights.contains(&item.0))
                .collect::<Vec<_>>();

            changed_highlights.sort();

            let changed_ranges = convert_consecutive_line_numbers_to_ranges(&changed_highlights);
            let changed_ranges = changed_ranges
                .into_iter()
                .map(|range| (range.start, range.end))
                .collect::<Vec<_>>();

            self.vim.exec(
                "clap#highlighter#add_ts_highlights",
                (bufnr, changed_ranges, diff_highlights),
            )?;
        } else {
            self.vim.exec(
                "clap#highlighter#add_ts_highlights",
                (bufnr, Vec::<(usize, usize)>::new(), &new_vim_highlights),
            )?;
        }

        Ok(Some(new_vim_highlights))
    }

    /// Refresh tree sitter highlights by reading the entire file and parsing it again.
    async fn refresh_tree_sitter_highlight(
        &mut self,
        bufnr: usize,
        language: Language,
    ) -> Result<(), PluginError> {
        let source_file = self.vim.bufabspath(bufnr).await?;
        let source_file = std::path::PathBuf::from(source_file);

        let source_code = std::fs::read(&source_file)?;

        let new_highlights = tree_sitter::highlight(language, &source_code)?;

        let (_winid, line_start, line_end) = self.vim.get_screen_lines_range().await?;

        let maybe_new_vim_highlights = self.apply_ts_highlights(
            bufnr,
            language,
            &new_highlights,
            Some(line_start - 1..line_end),
        )?;

        self.ts_bufs.entry(bufnr).and_modify(|i| {
            i.highlights = new_highlights.into();
            if let Some(new_vim_highlights) = maybe_new_vim_highlights {
                i.vim_highlights = new_vim_highlights;
            }
        });

        Ok(())
    }

    async fn tree_sitter_props_at_cursor(&mut self) -> Result<(), PluginError> {
        let (bufnr, row, column) = self.vim.get_cursor_pos().await?;

        if let Some(ts_info) = self.ts_bufs.get(&bufnr) {
            if let Some(props) =
                ts_info
                    .highlights
                    .syntax_props_at(ts_info.language, row - 1, column - 1)
            {
                self.vim.echo_message(format!("{props:?}"))?;
            } else {
                self.vim.echo_message("tree sitter props not found")?;
            }
        }

        Ok(())
    }
}

pub fn sublime_syntax_highlight<T: AsRef<str>>(
    syntax: &SyntaxReference,
    lines: impl Iterator<Item = T>,
    line_start_number: usize,
    theme: &str,
) -> Vec<(usize, Vec<TokenHighlight>)> {
    let highlighter = &SUBLIME_SYNTAX_HIGHLIGHTER;

    lines
        .enumerate()
        .filter_map(|(index, line)| {
            match highlighter.get_token_highlights_in_line(syntax, line.as_ref(), theme) {
                Ok(token_highlights) => Some((line_start_number + index, token_highlights)),
                Err(err) => {
                    tracing::error!(line = ?line.as_ref(), ?err, "Error at fetching line highlight");
                    None
                }
            }
        })
        .collect::<Vec<_>>()
}

#[async_trait::async_trait]
impl ClapPlugin for Syntax {
    #[maple_derive::subscriptions]
    async fn handle_autocmd(&mut self, autocmd: AutocmdEvent) -> Result<(), PluginError> {
        use AutocmdEventType::{BufDelete, BufEnter, BufWritePost, CursorMoved};

        if self.toggle.is_off() {
            return Ok(());
        }

        let (autocmd_event_type, params) = autocmd;
        let bufnr = params.parse_bufnr()?;

        match autocmd_event_type {
            BufEnter => self.on_buf_enter(bufnr).await?,
            BufWritePost => {
                if self.tree_sitter_enabled {
                    // if self.vim.bufmodified(bufnr).await? {
                    if let Some(ts_info) = self.ts_bufs.get(&bufnr) {
                        self.refresh_tree_sitter_highlight(bufnr, ts_info.language)
                            .await?;
                    }
                    //}
                }
            }
            BufDelete => {
                self.ts_bufs.remove(&bufnr);
                self.sublime_bufs.remove(&bufnr);
            }
            CursorMoved => {
                if self.tree_sitter_enabled {
                    if self.vim.bufmodified(bufnr).await? {
                        self.tree_sitter_highlight(bufnr, true, None).await?;
                    } else if let Some(ts_info) = self.ts_bufs.get(&bufnr) {
                        let (_winid, line_start, line_end) =
                            self.vim.get_screen_lines_range().await?;

                        self.apply_ts_highlights(
                            bufnr,
                            ts_info.language,
                            &ts_info.highlights.0,
                            Some(line_start - 1..line_end),
                        )?;
                    }
                } else if self.sublime_syntax_enabled {
                    self.sublime_syntax_highlight(bufnr).await?;
                }
            }
            event => return Err(PluginError::UnhandledEvent(event)),
        }

        Ok(())
    }

    async fn handle_action(&mut self, action: ActionRequest) -> Result<(), PluginError> {
        let ActionRequest { method, params: _ } = action;
        match self.parse_action(method)? {
            SyntaxAction::TreeSitterHighlight => {
                let bufnr = self.vim.bufnr("").await?;
                self.tree_sitter_highlight(bufnr, false, None).await?;
                self.tree_sitter_enabled = true;
                self.toggle.turn_on();
            }
            SyntaxAction::TreeSitterListScopes => {
                let bufnr = self.vim.bufnr("").await?;
                let extension = self.vim.expand(format!("#{bufnr}:p:e")).await?;
                if let Some(language) = tree_sitter::Language::try_from_extension(&extension) {
                    let highlight_scopes = tree_sitter::parse_scopes(language.highlight_query());
                    self.vim.echo_message(format!("{highlight_scopes:?}"))?;
                }
            }
            SyntaxAction::TreeSitterPropsAtCursor => {
                self.tree_sitter_props_at_cursor().await?;
            }
            SyntaxAction::ListSublimeThemes => {
                let highlighter = &SUBLIME_SYNTAX_HIGHLIGHTER;
                let theme_list = highlighter.get_theme_list();
                self.vim.echo_info(theme_list.into_iter().join(","))?;
            }
            SyntaxAction::SublimeSyntaxHighlight => {
                let bufnr = self.vim.bufnr("").await?;
                self.on_buf_enter(bufnr).await?;
                self.sublime_syntax_highlight(bufnr).await?;
                self.sublime_syntax_enabled = true;
            }
            SyntaxAction::Toggle => {
                match self.toggle {
                    Toggle::On => {}
                    Toggle::Off => {}
                }
                self.toggle.switch();
            }
        }

        Ok(())
    }
}

fn convert_consecutive_line_numbers_to_ranges(input: &[usize]) -> Vec<Range<usize>> {
    if input.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();

    let mut start = input[0];
    let mut end = input[0];

    for &num in &input[1..] {
        if num == end + 1 {
            end = num;
        } else {
            ranges.push(start..end + 1);
            start = num;
            end = num;
        }
    }

    ranges.push(start..end + 1);

    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_consecutive_line_numbers_to_ranges() {
        let input = [1, 2, 4, 5, 6, 7, 8, 9, 20, 23, 24];

        assert_eq!(
            vec![1..3, 4..10, 20..21, 23..25],
            convert_consecutive_line_numbers_to_ranges(&input)
        );

        let input = [1];
        assert_eq!(
            vec![1..2],
            convert_consecutive_line_numbers_to_ranges(&input)
        );
    }
}
