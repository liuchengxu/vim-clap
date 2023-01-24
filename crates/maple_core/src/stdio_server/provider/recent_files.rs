use crate::datastore::RECENT_FILES_IN_MEMORY;
use crate::paths::AbsPathBuf;
use crate::stdio_server::handler::CachedPreviewImpl;
use crate::stdio_server::provider::{ClapProvider, Context};
use anyhow::Result;
use matcher::ClapItem;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::sync::Arc;
use types::{MatchedItem, Score};

#[derive(Debug, Clone)]
pub struct RecentFilesProvider {
    lines: Arc<Mutex<Vec<MatchedItem>>>,
}

impl RecentFilesProvider {
    pub fn new() -> Self {
        Self {
            lines: Default::default(),
        }
    }

    fn process_query(
        self,
        cwd: AbsPathBuf,
        query: String,
        preview_size: Option<usize>,
        lnum: usize,
        winwidth: usize,
        icon: icon::Icon,
    ) -> Result<Value> {
        let mut recent_files = RECENT_FILES_IN_MEMORY.lock();
        let cwd = cwd.to_string();

        let ranked = if query.is_empty() {
            // Sort the initial list according to the cwd.
            //
            // This changes the order of existing recent file entries.
            recent_files.sort_by_cwd(&cwd);

            let mut cwd = cwd.clone();
            cwd.push(std::path::MAIN_SEPARATOR);

            recent_files
                .entries
                .iter()
                .map(|entry| {
                    let item: Arc<dyn ClapItem> = Arc::new(entry.fpath.replacen(&cwd, "", 1));
                    // frecent_score will not be larger than i32::MAX.
                    MatchedItem::new(item, entry.frecent_score as Score, Default::default())
                })
                .collect::<Vec<_>>()
        } else {
            recent_files.filter_on_query(&query, cwd.clone())
        };

        let processed = recent_files.len();
        let matched = ranked.len();

        // process the new preview
        let preview = match (preview_size, ranked.get(lnum - 1)) {
            (Some(size), Some(new_entry)) => {
                let new_curline = new_entry.display_text().to_string();
                if let Ok((lines, fname)) =
                    crate::previewer::preview_file(new_curline, size, winwidth)
                {
                    Some(json!({ "lines": lines, "fname": fname }))
                } else {
                    None
                }
            }
            _ => None,
        };

        let printer::DisplayLines {
            lines,
            indices,
            truncated_map,
            icon_added,
        } = printer::to_display_lines(ranked.iter().take(200).cloned().collect(), winwidth, icon);

        let mut cwd = cwd;
        cwd.push(std::path::MAIN_SEPARATOR);

        let lines = lines
            .into_iter()
            .map(|abs_path| abs_path.replacen(&cwd, "", 1))
            .collect::<Vec<_>>();

        let response = if truncated_map.is_empty() {
            json!({
                "lines": lines,
                "indices": indices,
                "matched": matched,
                "processed": processed,
                "icon_added": icon_added,
                "preview": preview,
            })
        } else {
            json!({
                "lines": lines,
                "indices": indices,
                "truncated_map": truncated_map,
                "matched": matched,
                "processed": processed,
                "icon_added": icon_added,
                "preview": preview,
            })
        };

        let mut lines = self.lines.lock();
        *lines = ranked;

        Ok(response)
    }
}

#[async_trait::async_trait]
impl ClapProvider for RecentFilesProvider {
    async fn on_initialize(&mut self, ctx: &mut Context) -> Result<()> {
        let query = ctx.vim.context_query_or_input().await?;
        let cwd = ctx.vim.working_dir().await?;

        let preview_size = if ctx.env.preview_enabled {
            Some(ctx.preview_size().await?)
        } else {
            None
        };

        let winwidth = ctx.env.display_winwidth;
        let icon = if ctx.env.icon.enabled() {
            icon::Icon::Enabled(icon::IconKind::File)
        } else {
            icon::Icon::Null
        };

        let response = self
            .clone()
            .process_query(cwd, query, preview_size, 1, winwidth, icon)?;

        ctx.vim
            .call("clap#state#process_response_on_typed", response)
            .await?;

        Ok(())
    }

    async fn on_move(&mut self, ctx: &mut Context) -> Result<()> {
        let lnum = ctx.vim.display_getcurlnum().await?;

        let maybe_curline = self
            .lines
            .lock()
            .get(lnum - 1)
            .map(|r| r.item.raw_text().to_string());

        if let Some(curline) = maybe_curline {
            let preview_height = ctx.preview_height().await?;
            let preview = CachedPreviewImpl::new(curline, preview_height, ctx)?
                .get_preview()
                .await?;
            ctx.render_preview(preview)?;
        }

        Ok(())
    }

    async fn on_typed(&mut self, ctx: &mut Context) -> Result<()> {
        let query = ctx.vim.input_get().await?;

        let response = tokio::task::spawn_blocking({
            let query = query.clone();
            let recent_files = self.clone();

            let cwd = ctx.cwd.clone();
            let preview_size = if ctx.env.preview_enabled {
                Some(ctx.preview_size().await?)
            } else {
                None
            };
            let lnum = ctx.vim.display_getcurlnum().await?;
            let winwidth = ctx.env.display_winwidth;
            let icon = if ctx.env.icon.enabled() {
                icon::Icon::Enabled(icon::IconKind::File)
            } else {
                icon::Icon::Null
            };

            move || recent_files.process_query(cwd, query, preview_size, lnum, winwidth, icon)
        })
        .await??;

        let current_query = ctx.vim.input_get().await?;
        if current_query == query {
            ctx.vim
                .call("clap#state#process_response_on_typed", response)
                .await?;
        }

        Ok(())
    }
}
