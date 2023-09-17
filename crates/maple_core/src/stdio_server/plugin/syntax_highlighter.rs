use crate::stdio_server::input::{AutocmdEventType, PluginEvent};
use crate::stdio_server::plugin::{
    Action, ActionType, ClapAction, ClapPlugin, PluginAction, PluginId, Toggle,
};
use crate::stdio_server::vim::Vim;
use anyhow::{anyhow, Result};
use itertools::Itertools;
use once_cell::sync::Lazy;

static HIGHLIGHTER: Lazy<highlighter::SyntaxHighlighter> =
    Lazy::new(highlighter::SyntaxHighlighter::new);

#[derive(Debug, Clone)]
pub struct SyntaxHighlighterPlugin {
    vim: Vim,
    toggle: Toggle,
}

// TODO: use a derive macro.
impl SyntaxHighlighterPlugin {
    const SYNTAX_ON: &'static str = "syntax/on";
    const SYNTAX_ON_ACTION: Action = Action::callable(Self::SYNTAX_ON);

    const LIST_THEMES: &'static str = "syntax/list-themes";
    const LIST_THEMES_ACTION: Action = Action::callable(Self::LIST_THEMES);

    const TOGGLE: &'static str = "syntax/toggle";
    const TOGGLE_ACTION: Action = Action::callable(Self::TOGGLE);

    const ACTIONS: &[Action] = &[
        Self::SYNTAX_ON_ACTION,
        Self::LIST_THEMES_ACTION,
        Self::TOGGLE_ACTION,
    ];
}

impl ClapAction for SyntaxHighlighterPlugin {
    fn actions(&self, _action_type: ActionType) -> &[Action] {
        Self::ACTIONS
    }
}

impl SyntaxHighlighterPlugin {
    pub const ID: PluginId = PluginId::SyntaxHighlighter;

    pub fn new(vim: Vim) -> Self {
        Self {
            vim,
            toggle: Toggle::On,
        }
    }

    async fn on_buf_enter(&mut self, bufnr: usize) -> Result<()> {
        Ok(())
    }

    // TODO: this may be inaccurate, e.g., the lines are part of a bigger block of comments.
    async fn highlight_visual_lines(&self, bufnr: usize) -> anyhow::Result<()> {
        let fpath = self.vim.current_buffer_path().await?;
        let Some(extension) = std::path::Path::new(&fpath)
            .extension()
            .and_then(|e| e.to_str())
        else {
            return Ok(());
        };

        let line_start = self.vim.line("w0").await?;
        let end = self.vim.line("w$").await?;
        let lines = self.vim.getbufline(bufnr, line_start, end).await?;

        let highlighter = &HIGHLIGHTER;

        tracing::debug!(
            "=========== themes: {:?}, fg: {:?}",
            highlighter.theme_set.themes.keys(),
            highlighter.theme_set.themes["Coldark-Dark"]
                .settings
                .foreground
        );

        let syntax = highlighter
            .syntax_set
            .find_syntax_by_extension(extension)
            .ok_or_else(|| anyhow!("Can not find syntax for extension {extension}"))?;

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
        let line_highlights = lines
            .iter()
            .enumerate()
            .filter_map(|(idx, line)| {
                match highlighter.get_token_highlights_in_line(syntax, line, THEME) {
                    Ok(token_highlights) => Some((line_start + idx, token_highlights)),
                    Err(err) => {
                        tracing::error!(?line, ?err, "Error at fetching line highlight");
                        None
                    }
                }
            })
            .collect::<Vec<_>>();

        self.vim
            .exec("clap#highlighter#highlight_lines", (bufnr, line_highlights))?;

        tracing::debug!("Lines highlight elapsed: {:?}ms", now.elapsed().as_millis());

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClapPlugin for SyntaxHighlighterPlugin {
    fn id(&self) -> PluginId {
        Self::ID
    }

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
                    BufDelete => {}
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
                    Self::SYNTAX_ON => {
                        let bufnr = self.vim.bufnr("").await?;
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
