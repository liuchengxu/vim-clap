use crate::stdio_server::plugin::PluginError;
use crate::stdio_server::Vim;
use itertools::Itertools;
use once_cell::sync::Lazy;
use sublime_syntax::{SyntaxReference, TokenHighlight};

static SUBLIME_SYNTAX_HIGHLIGHTER: Lazy<sublime_syntax::SyntaxHighlighter> =
    Lazy::new(sublime_syntax::SyntaxHighlighter::new);

pub fn sublime_theme_exists(theme: &str) -> bool {
    SUBLIME_SYNTAX_HIGHLIGHTER.theme_exists(theme)
}

pub fn sublime_syntax_by_extension(extension: &str) -> Option<&SyntaxReference> {
    SUBLIME_SYNTAX_HIGHLIGHTER
        .syntax_set
        .find_syntax_by_extension(extension)
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

#[derive(Debug, Clone)]
pub struct SublimeSyntaxImpl {
    vim: Vim,
}

impl SublimeSyntaxImpl {
    pub fn new(vim: Vim) -> Self {
        Self { vim }
    }

    pub fn list_themes(&self) -> Result<(), PluginError> {
        let highlighter = &SUBLIME_SYNTAX_HIGHLIGHTER;
        let theme_list = highlighter.get_theme_list();
        self.vim.echo_info(theme_list.into_iter().join(","))?;
        Ok(())
    }

    /// Highlight the visual lines of specified buffer.
    // TODO: this may be inaccurate, e.g., the highlighted lines are part of a bigger block of comments.
    pub async fn do_highlight(&mut self, bufnr: usize, extension: &str) -> Result<(), PluginError> {
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
            "clap#highlighter#add_sublime_highlights",
            (bufnr, &line_highlights),
        )?;

        tracing::debug!("Lines highlight elapsed: {:?}ms", now.elapsed().as_millis());

        Ok(())
    }
}
