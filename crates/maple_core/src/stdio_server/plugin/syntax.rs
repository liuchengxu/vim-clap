use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::plugin::{ActionRequest, ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::{Vim, VimResult};
use itertools::Itertools;
use once_cell::sync::Lazy;
use sublime_syntax::{SyntaxReference, TokenHighlight};

pub static SUBLIME_SYNTAX_HIGHLIGHTER: Lazy<sublime_syntax::SyntaxHighlighter> =
    Lazy::new(sublime_syntax::SyntaxHighlighter::new);

const HIGHLIGHT_NAMES: &[(&str, &str)] = &[
    ("comment", "Comment"),
    ("constant", "Constant"),
    ("constant.builtin", "Constant"),
    ("function", "Function"),
    ("function.builtin", "Special"),
    ("function.macro", "Macro"),
    ("keyword", "Keyword"),
    ("operator", "Operator"),
    ("property", "Identifier"),
    ("punctuation.delimiter", "Delimiter"),
    ("string", "String"),
    ("string.special", "SpecialChar"),
    ("type", "Type"),
    ("type.definition", "Typedef"),
    ("type.builtin", "Type"),
    ("tag", "Tag"),
    ("attribute", "Conditional"),
    ("conditional", "Conditional"),
    ("punctuation", "Delimiter"),
    ("punctuation.bracket", "Delimiter"),
    ("variable", "Identifier"),
    ("variable.builtin", "Identifier"),
    ("variable.parameter", "Identifier"),
];

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
    fn syntax_props_at(&self, row: usize, column: usize) -> Option<SyntaxProps> {
        self.0.get(&row).and_then(|highlights| {
            highlights.iter().find_map(|h| {
                if (h.start.column..h.end.column).contains(&column) {
                    Some(SyntaxProps {
                        row: h.start.row,
                        range: h.start.column..h.end.column,
                        length: h.end.column - h.start.column,
                        node: HIGHLIGHT_NAMES[h.highlight.0].0,
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

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(id = "syntax", actions = ["on", "tree-sitter-props-at-cursor", "tree-sitter-highlight", "list-themes", "toggle"])]
pub struct Syntax {
    vim: Vim,
    bufs: HashMap<usize, String>,
    tree_sitter_highlights: HashMap<usize, BufferHighlights>,
    toggle: Toggle,
}

impl Syntax {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            bufs: HashMap::new(),
            tree_sitter_highlights: HashMap::new(),
            toggle: Toggle::Off,
        }
    }

    async fn on_buf_enter(&mut self, bufnr: usize) -> VimResult<()> {
        let fpath = self.vim.bufabspath(bufnr).await?;
        if let Some(extension) = std::path::Path::new(&fpath)
            .extension()
            .and_then(|e| e.to_str())
        {
            self.bufs.insert(bufnr, extension.to_string());
        }

        Ok(())
    }

    /// Highlight the visual lines of specified buffer.
    // TODO: this may be inaccurate, e.g., the lines are part of a bigger block of comments.
    async fn syntect_highlight(&mut self, bufnr: usize) -> Result<(), PluginError> {
        let Some(extension) = self.bufs.get(&bufnr) else {
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

        tracing::debug!(
            "=========== themes: {:?}, fg: {:?}",
            highlighter.theme_set.themes.keys(),
            highlighter.theme_set.themes["Coldark-Dark"]
                .settings
                .foreground
        );

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

        // TODO: Clear the outdated highlights first and then render the new highlights.
        self.vim.exec(
            "clap#highlighter#highlight_lines",
            (bufnr, &line_highlights),
        )?;

        tracing::debug!("Lines highlight elapsed: {:?}ms", now.elapsed().as_millis());

        Ok(())
    }

    async fn tree_sitter_highlight(&mut self) -> Result<(), PluginError> {
        let bufnr = self.vim.bufnr("").await?;
        let source_file = self.vim.bufabspath(bufnr).await?;
        let source_file = std::path::PathBuf::from(source_file);

        let source_code = std::fs::read_to_string(&source_file).unwrap();

        let Some(language) = source_file.extension().and_then(|e| {
            e.to_str()
                .and_then(|extension| tree_sitter::Language::try_from_extension(extension))
        }) else {
            // Enable vim regex syntax highlighting.
            self.vim.exec("execute", "syntax on")?;
            return Ok(());
        };

        if self.vim.eval::<usize>("exists('g:syntax_on')").await? != 0 {
            self.vim.exec("execute", "syntax off")?;
        }

        // TODO: efficient SyntaxHighlighter
        let mut tree_sitter_highlighter = tree_sitter::SyntaxHighlighter::new();
        let buffer_highlights = tree_sitter_highlighter.highlight(
            language,
            source_code.as_bytes(),
            &HIGHLIGHT_NAMES.iter().map(|(h, _)| *h).collect::<Vec<_>>(),
        )?;

        let mut vim_highlights = Vec::new();

        for (line_number, highlight_items) in &buffer_highlights {
            let line_highlights: Vec<(usize, usize, &str)> = highlight_items
                .iter()
                .map(|i| {
                    (
                        i.start.column,
                        i.end.column - i.start.column,
                        *&HIGHLIGHT_NAMES[i.highlight.0].1,
                    )
                })
                .collect();

            vim_highlights.push((line_number, line_highlights));
        }

        self.vim.exec(
            "clap#highlighter#add_line_highlights",
            (bufnr, vim_highlights),
        )?;

        self.tree_sitter_highlights
            .insert(bufnr, buffer_highlights.into());

        Ok(())
    }

    async fn tree_sitter_props_at_cursor(&mut self) -> Result<(), PluginError> {
        let (bufnr, row, column) = self.vim.get_cursor_pos().await?;

        if let Some(buf_highlights) = self.tree_sitter_highlights.get(&bufnr) {
            if let Some(props) = buf_highlights.syntax_props_at(row - 1, column - 1) {
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
            BufWritePost => {}
            BufDelete => {
                self.bufs.remove(&bufnr);
            }
            CursorMoved => {
                self.syntect_highlight(bufnr).await?;
            }
            event => return Err(PluginError::UnhandledEvent(event)),
        }

        Ok(())
    }

    async fn handle_action(&mut self, action: ActionRequest) -> Result<(), PluginError> {
        let ActionRequest { method, params: _ } = action;
        match self.parse_action(method)? {
            SyntaxAction::On => {
                let bufnr = self.vim.bufnr("").await?;
                self.on_buf_enter(bufnr).await?;
                self.syntect_highlight(bufnr).await?;
            }
            SyntaxAction::TreeSitterHighlight => {
                self.tree_sitter_highlight().await?;
            }
            SyntaxAction::TreeSitterPropsAtCursor => {
                self.tree_sitter_props_at_cursor().await?;
            }
            SyntaxAction::ListThemes => {
                let highlighter = &SUBLIME_SYNTAX_HIGHLIGHTER;
                let theme_list = highlighter.get_theme_list();
                self.vim.echo_info(theme_list.into_iter().join(","))?;
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
