use std::collections::HashMap;

use crate::stdio_server::input::{AutocmdEvent, AutocmdEventType};
use crate::stdio_server::plugin::{ActionRequest, ClapPlugin, PluginError, Toggle};
use crate::stdio_server::vim::{Vim, VimResult};
use highlighter::{SyntaxReference, TokenHighlight};
use itertools::Itertools;
use once_cell::sync::Lazy;

pub static HIGHLIGHTER: Lazy<highlighter::SyntaxHighlighter> =
    Lazy::new(highlighter::SyntaxHighlighter::new);

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(id = "syntax", actions = ["on", "list-themes", "toggle"])]
pub struct Syntax {
    vim: Vim,
    bufs: HashMap<usize, String>,
    toggle: Toggle,
}

impl Syntax {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            bufs: HashMap::new(),
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

    // TODO: this may be inaccurate, e.g., the lines are part of a bigger block of comments.
    async fn highlight_visual_lines(&mut self, bufnr: usize) -> Result<(), PluginError> {
        let Some(extension) = self.bufs.get(&bufnr) else {
            return Ok(());
        };

        let highlighter = &HIGHLIGHTER;
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
        let line_highlights = highlight_lines(syntax, lines.iter(), line_start, THEME);

        // TODO: Clear the outdated highlights first and then render the new highlights.
        self.vim.exec(
            "clap#highlighter#highlight_lines",
            (bufnr, &line_highlights),
        )?;

        tracing::debug!("Lines highlight elapsed: {:?}ms", now.elapsed().as_millis());

        Ok(())
    }
}

pub fn highlight_lines<T: AsRef<str>>(
    syntax: &SyntaxReference,
    lines: impl Iterator<Item = T>,
    line_start_number: usize,
    theme: &str,
) -> Vec<(usize, Vec<TokenHighlight>)> {
    let highlighter = &HIGHLIGHTER;

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
                self.highlight_visual_lines(bufnr).await?;
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
                self.highlight_visual_lines(bufnr).await?;
            }
            SyntaxAction::ListThemes => {
                let highlighter = &HIGHLIGHTER;
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
