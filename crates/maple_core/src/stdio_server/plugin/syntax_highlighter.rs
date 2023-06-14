use crate::stdio_server::vim::Vim;
use once_cell::sync::Lazy;

static HIGHLIGHTER: Lazy<highlighter::SyntaxHighlighter> =
    Lazy::new(highlighter::SyntaxHighlighter::new);

#[derive(Debug)]
pub struct SyntaxHighlighter {
    vim: Vim,
}

impl SyntaxHighlighter {
    pub fn new(vim: Vim) -> Self {
        Self { vim }
    }

    // TODO: this may be inaccurate, e.g., the lines are part of a bigger block of comments.
    pub async fn highlight_visual_lines(&self) -> anyhow::Result<()> {
        let lnum = self.vim.line("w0").await?;
        let end = self.vim.line("w$").await?;
        let bufnr = self.vim.bufnr("").await?;
        let lines = self.vim.getbufline(bufnr, lnum, end).await?;

        let fpath = self.vim.current_buffer_path().await?;
        let Some(extension) = std::path::Path::new(&fpath)
                    .extension()
                    .and_then(|e| e.to_str()) else {
                        return Ok(())
                    };

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
            .ok_or_else(|| anyhow::anyhow!("Can not find syntax for extension {extension}"))?;

        const THEME: &str = "Coldark-Dark";

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
                    Ok(token_highlights) => Some((lnum + idx, token_highlights)),
                    Err(err) => {
                        tracing::error!(?line, ?err, "Error at fetching line highlight");
                        None
                    }
                }
            })
            .collect::<Vec<_>>();
        self.vim.exec(
            "clap#highlighter#highlight_lines",
            serde_json::json!([bufnr, line_highlights]),
        )?;

        tracing::debug!("Lines highlight elapsed: {:?}ms", now.elapsed().as_millis());
        Ok(())
    }
}
