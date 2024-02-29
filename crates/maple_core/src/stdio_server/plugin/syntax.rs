pub mod sublime;

use self::sublime::SublimeSyntaxImpl;
use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::plugin::{ClapPlugin, PluginAction, PluginError, Toggle};
use crate::stdio_server::vim::Vim;
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::path::{Path, PathBuf};
use tree_sitter::Language;

#[allow(unused)]
#[derive(Debug)]
struct SyntaxProps {
    row: usize,
    range: Range<usize>,
    length: usize,
    node: &'static str,
    highlight_group: &'static str,
}

type RawTsHighlights = BTreeMap<usize, Vec<tree_sitter::HighlightItem>>;

/// Represents the tree-sitter highlight info of entire buffer.
#[derive(Debug, Clone)]
struct BufferHighlights(RawTsHighlights);

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
                        highlight_group: language.highlight_group(h.highlight),
                    })
                } else {
                    None
                }
            })
        })
    }
}

impl From<RawTsHighlights> for BufferHighlights {
    fn from(inner: RawTsHighlights) -> Self {
        Self(inner)
    }
}

/// (start, length, highlight_group)
type LineHighlights = Vec<(usize, usize, &'static str)>;
type VimHighlights = Vec<(usize, LineHighlights)>;

#[derive(Debug, Clone)]
struct TreeSitterInfo {
    language: Language,
    /// Used to infer the highlighting render strategy.
    file_size: FileSize,
    /// Highlights of entire buffer.
    highlights: BufferHighlights,
    /// Current vim highlighting info, note that we only
    /// highlight the visual lines on the vim side.
    vim_highlights: VimHighlights,
}

/// File size in bytes.
#[derive(Debug, Clone, Copy)]
struct FileSize(usize);

impl std::fmt::Display for FileSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 < 1024 {
            write!(f, "{}bytes", self.0)
        } else if self.0 < 1024 * 1024 {
            write!(f, "{}KiB", self.0 / 1024)
        } else {
            write!(f, "{}MiB", self.0 / 1024 / 1024)
        }
    }
}

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(
  id = "syntax",
  actions = [
    "sublimeSyntaxHighlight",
    "sublimeSyntaxListThemes",
    "treeSitterHighlight",
    "treeSitterHighlightDisable",
    "treeSitterListScopes",
    "treeSitterPropsAtCursor",
    "toggle",
  ],
)]
pub struct Syntax {
    vim: Vim,
    toggle: Toggle,
    ts_bufs: HashMap<usize, TreeSitterInfo>,
    sublime_bufs: HashMap<usize, String>,
    sublime_impl: SublimeSyntaxImpl,
    tree_sitter_enabled: bool,
    sublime_syntax_enabled: bool,
}

impl Syntax {
    pub fn new(vim: Vim) -> Self {
        let sublime_impl = SublimeSyntaxImpl::new(vim.clone());
        Self {
            vim,
            toggle: Toggle::Off,
            ts_bufs: HashMap::new(),
            sublime_bufs: HashMap::new(),
            sublime_impl,
            tree_sitter_enabled: false,
            sublime_syntax_enabled: false,
        }
    }

    async fn on_buf_enter(&mut self, bufnr: usize) -> Result<(), PluginError> {
        let fpath = self.vim.bufabspath(bufnr).await?;
        let maybe_extension = Path::new(&fpath).extension().and_then(|e| e.to_str());

        if let Some(extension) = maybe_extension {
            self.sublime_bufs.insert(bufnr, extension.to_string());
        }

        if self.tree_sitter_enabled {
            if let Some(language) =
                maybe_extension.and_then(tree_sitter::Language::try_from_extension)
            {
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

        Ok(())
    }

    async fn identify_buffer_language(&self, bufnr: usize, source_file: &Path) -> Option<Language> {
        if let Some(language) = tree_sitter::Language::try_from_path(source_file) {
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
        let source_file = PathBuf::from(self.vim.bufabspath(bufnr).await?);

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
            // TODO: this requests the entire buffer content, which might be performance sensitive
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

        let start = std::time::Instant::now();

        let raw_highlights = language.highlight(&source_code)?;

        let file_size = FileSize(source_code.len());

        tracing::trace!(
            ?language,
            highlighted_lines = raw_highlights.len(),
            %file_size,
            "fetching tree-sitter highlighting info elapsed: {:?}ms",
            start.elapsed().as_millis()
        );

        let maybe_vim_highlights = self
            .render_ts_highlights(bufnr, language, &raw_highlights, file_size)
            .await?;

        self.ts_bufs.insert(
            bufnr,
            TreeSitterInfo {
                language,
                highlights: raw_highlights.into(),
                vim_highlights: maybe_vim_highlights.unwrap_or_default(),
                file_size,
            },
        );

        Ok(())
    }

    /// Returns Some() if the vim highlights are changed.
    async fn render_ts_highlights(
        &self,
        bufnr: usize,
        language: Language,
        raw_ts_highlights: &RawTsHighlights,
        file_size: FileSize,
    ) -> Result<Option<VimHighlights>, PluginError> {
        use maple_config::RenderStrategy;

        let render_strategy = &maple_config::config().plugin.syntax.render_strategy;

        let highlight_range = match render_strategy {
            RenderStrategy::VisualLines => {
                let (_winid, line_start, line_end) = self.vim.get_screen_lines_range().await?;
                HighlightRange::Lines(line_start - 1..line_end)
            }
            RenderStrategy::EntireBufferUpToLimit(size_limit) => {
                if file_size.0 <= *size_limit {
                    HighlightRange::EveryLine
                } else {
                    let (_winid, line_start, line_end) = self.vim.get_screen_lines_range().await?;
                    HighlightRange::Lines(line_start - 1..line_end)
                }
            }
        };

        let new_vim_highlights = convert_raw_ts_highlights_to_vim_highlights(
            raw_ts_highlights,
            language,
            highlight_range,
        );

        if let Some(old) = self.ts_bufs.get(&bufnr) {
            let old_vim_highlights = &old.vim_highlights;

            let (unchanged_highlights, changed_highlights): (Vec<_>, Vec<_>) = new_vim_highlights
                .iter()
                .partition(|item| old_vim_highlights.contains(item));

            let unchanged_lines = unchanged_highlights
                .into_iter()
                .map(|item| item.0)
                .collect::<Vec<_>>();

            let mut changed_lines = changed_highlights
                .into_iter()
                .map(|item| item.0)
                .collect::<Vec<_>>();

            // No new highlight changes since the last highlighting operation.
            if changed_lines.is_empty() {
                return Ok(None);
            }

            tracing::trace!(
                total = new_vim_highlights.len(),
                unchanged_lines_count = unchanged_lines.len(),
                changed_lines_count = changed_lines.len(),
                "Applying new highlight changes"
            );
            tracing::trace!(
                "unchanged lines: {unchanged_lines:?}, changed lines: {changed_lines:?}"
            );

            // Keep the changed highlights only.
            let diff_highlights = new_vim_highlights
                .iter()
                .filter(|item| changed_lines.contains(&item.0))
                .collect::<Vec<_>>();

            changed_lines.sort();

            let changed_ranges = convert_consecutive_line_numbers_to_ranges(&changed_lines);
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
        let source_file = PathBuf::from(self.vim.bufabspath(bufnr).await?);

        let source_code = std::fs::read(&source_file)?;

        let new_highlights = language.highlight(&source_code)?;

        let file_size = FileSize(source_code.len());

        let maybe_new_vim_highlights = self
            .render_ts_highlights(bufnr, language, &new_highlights, file_size)
            .await?;

        self.ts_bufs.entry(bufnr).and_modify(|i| {
            i.highlights = new_highlights.into();
            i.file_size = file_size;
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
        } else {
            self.vim
                .echo_message("tree sitter highlight not enabled for this buffer")?;
        }

        Ok(())
    }
}

pub enum HighlightRange {
    /// Only highlight the lines in the specified range.
    Lines(Range<usize>),
    /// All lines will be highlighted.
    EveryLine,
}

impl From<Range<usize>> for HighlightRange {
    fn from(range: Range<usize>) -> Self {
        Self::Lines(range)
    }
}

impl HighlightRange {
    /// Returns `true` if the line at specified line number should be highlighted.
    fn should_highlight(&self, line_number: usize) -> bool {
        match self {
            Self::Lines(range) => range.contains(&line_number),
            Self::EveryLine => true,
        }
    }
}

/// Convert the raw highlight info to something that is directly applied by Vim.
pub fn convert_raw_ts_highlights_to_vim_highlights(
    raw_ts_highlights: &RawTsHighlights,
    language: Language,
    highlight_range: HighlightRange,
) -> VimHighlights {
    raw_ts_highlights
        .iter()
        .filter(|(line_number, _)| highlight_range.should_highlight(**line_number))
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
                    if let Some(ts_info) = self.ts_bufs.get(&bufnr) {
                        self.refresh_tree_sitter_highlight(bufnr, ts_info.language)
                            .await?;
                    }
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
                    } else {
                        let maybe_new_vim_highlights =
                            if let Some(ts_info) = self.ts_bufs.get(&bufnr) {
                                self.render_ts_highlights(
                                    bufnr,
                                    ts_info.language,
                                    &ts_info.highlights.0,
                                    ts_info.file_size,
                                )
                                .await?
                            } else {
                                None
                            };
                        if let Some(new_vim_highlights) = maybe_new_vim_highlights {
                            self.ts_bufs.entry(bufnr).and_modify(|i| {
                                i.vim_highlights = new_vim_highlights;
                            });
                        }
                    }
                } else if self.sublime_syntax_enabled {
                    if let Some(extension) = self.sublime_bufs.get(&bufnr) {
                        self.sublime_impl.do_highlight(bufnr, extension).await?;
                        return Ok(());
                    };
                }
            }
            event => return Err(PluginError::UnhandledEvent(event)),
        }

        Ok(())
    }

    async fn handle_action(&mut self, action: PluginAction) -> Result<(), PluginError> {
        let PluginAction { method, params: _ } = action;
        match self.parse_action(method)? {
            SyntaxAction::TreeSitterHighlight => {
                let bufnr = self.vim.bufnr("").await?;
                self.tree_sitter_highlight(bufnr, false, None).await?;
                self.tree_sitter_enabled = true;
                self.toggle.turn_on();
            }
            SyntaxAction::TreeSitterHighlightDisable => {
                let bufnr = self.vim.bufnr("").await?;
                self.vim
                    .exec("clap#highlighter#disable_tree_sitter", bufnr)?;
                self.tree_sitter_enabled = false;
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
            SyntaxAction::SublimeSyntaxHighlight => {
                let bufnr = self.vim.bufnr("").await?;
                self.on_buf_enter(bufnr).await?;
                if let Some(extension) = self.sublime_bufs.get(&bufnr) {
                    self.sublime_impl.do_highlight(bufnr, extension).await?;
                }
                self.sublime_syntax_enabled = true;
            }
            SyntaxAction::SublimeSyntaxListThemes => {
                self.sublime_impl.list_themes()?;
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
