use std::collections::HashMap;

use crate::stdio_server::input::{AutocmdEventType, PluginEvent};
use crate::stdio_server::plugin::{ClapPlugin, PluginAction, Toggle};
use crate::stdio_server::vim::Vim;
use anyhow::{anyhow, Result};
use highlighter::{SyntaxReference, TokenHighlight};
use itertools::Itertools;
use once_cell::sync::Lazy;

pub static HIGHLIGHTER: Lazy<highlighter::SyntaxHighlighter> =
    Lazy::new(highlighter::SyntaxHighlighter::new);

#[derive(Debug, Clone, maple_derive::ClapPlugin)]
#[clap_plugin(id = "syntax", actions = ["on", "list-themes", "toggle"])]
pub struct SyntaxHighlighterPlugin {
    vim: Vim,
    bufs: HashMap<usize, String>,
    toggle: Toggle,
}

impl SyntaxHighlighterPlugin {
    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            bufs: HashMap::new(),
            toggle: Toggle::Off,
        }
    }

    async fn on_buf_enter(&mut self, bufnr: usize) -> Result<()> {
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
    async fn highlight_visual_lines(&mut self, bufnr: usize) -> anyhow::Result<()> {
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
        let line_highlights = highlight_lines(syntax, &lines, line_start, THEME);

        // TODO: Clear the outdated highlights first and then render the new highlights.
        self.vim.exec(
            "clap#highlighter#highlight_lines",
            (bufnr, &line_highlights),
        )?;

        tracing::debug!("Lines highlight elapsed: {:?}ms", now.elapsed().as_millis());

        Ok(())
    }
}

pub fn highlight_lines(
    syntax: &SyntaxReference,
    lines: &[String],
    line_start_number: usize,
    theme: &str,
) -> Vec<(usize, Vec<TokenHighlight>)> {
    let highlighter = &HIGHLIGHTER;

    lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            match highlighter.get_token_highlights_in_line(syntax, line, theme) {
                Ok(token_highlights) => Some((line_start_number + index, token_highlights)),
                Err(err) => {
                    tracing::error!(?line, ?err, "Error at fetching line highlight");
                    None
                }
            }
        })
        .collect::<Vec<_>>()
}

#[async_trait::async_trait]
impl ClapPlugin for SyntaxHighlighterPlugin {
    async fn on_plugin_event(&mut self, plugin_event: PluginEvent) -> Result<()> {
        match plugin_event {
            PluginEvent::Autocmd((autocmd_event_type, params)) => {
                use AutocmdEventType::{BufDelete, BufEnter, BufWritePost, CursorMoved};

                if self.toggle.is_off() {
                    return Ok(());
                }

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
                    _ => {}
                }

                Ok(())
            }
            PluginEvent::Action(plugin_action) => {
                let PluginAction { method, params: _ } = plugin_action;
                match method.as_str() {
                    Self::ON => {
                        let bufnr = self.vim.bufnr("").await?;
                        self.on_buf_enter(bufnr).await?;
                        self.highlight_visual_lines(bufnr).await?;
                    }
                    Self::LIST_THEMES => {
                        let highlighter = &HIGHLIGHTER;
                        let theme_list = highlighter.get_theme_list();
                        self.vim.echo_info(theme_list.into_iter().join(","))?;
                    }
                    Self::TOGGLE => {
                        match self.toggle {
                            Toggle::On => {}
                            Toggle::Off => {}
                        }
                        self.toggle.switch();
                    }
                    unknown_action => return Err(anyhow!("Unknown action: {unknown_action:?}")),
                }

                Ok(())
            }
        }
    }
}
