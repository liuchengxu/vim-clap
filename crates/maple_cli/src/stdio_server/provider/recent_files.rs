use crate::datastore::RECENT_FILES_IN_MEMORY;
use crate::paths::AbsPathBuf;
use crate::stdio_server::handler::PreviewImpl;
use crate::stdio_server::provider::{ClapProvider, ProviderContext};
use crate::stdio_server::vim::Vim;
use anyhow::Result;
use matcher::ClapItem;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::sync::Arc;
use types::{MatchedItem, Score};

#[derive(Debug, Clone)]
pub struct RecentFilesProvider {
    context: Arc<ProviderContext>,
    lines: Arc<Mutex<Vec<MatchedItem>>>,
}

impl RecentFilesProvider {
    pub fn new(context: ProviderContext) -> Self {
        Self {
            context: Arc::new(context),
            lines: Default::default(),
        }
    }

    #[inline]
    fn vim(&self) -> &Vim {
        &self.context.vim
    }

    fn process_query(
        self,
        cwd: AbsPathBuf,
        query: String,
        preview_size: usize,
        lnum: usize,
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
        let initial_size = recent_files.len();

        let total = ranked.len();

        let winwidth = self.context.env.display_winwidth;

        // process the new preview
        let preview = if let Some(new_entry) = ranked.get(lnum - 1) {
            let new_curline = new_entry.display_text().to_string();
            if let Ok((lines, fname)) =
                crate::previewer::preview_file(new_curline, preview_size, winwidth)
            {
                Some(json!({ "lines": lines, "fname": fname }))
            } else {
                None
            }
        } else {
            None
        };

        // Take the first 200 entries and add an icon to each of them.
        let printer::DisplayLines {
            lines,
            indices,
            truncated_map,
            icon_added,
        } = printer::decorate_lines(
            ranked.iter().take(200).cloned().collect(),
            winwidth,
            if self.context.env.icon.enabled() {
                icon::Icon::Enabled(icon::IconKind::File)
            } else {
                icon::Icon::Null
            },
        );

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
                "total": total,
                "icon_added": icon_added,
                "initial_size": initial_size,
                "preview": preview,
            })
        } else {
            json!({
                "lines": lines,
                "indices": indices,
                "truncated_map": truncated_map,
                "total": total,
                "icon_added": icon_added,
                "initial_size": initial_size,
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
    fn context(&self) -> &ProviderContext {
        &self.context
    }

    async fn on_create(&mut self) -> Result<()> {
        let query = self.vim().context_query_or_input().await?;
        let cwd = self.vim().working_dir().await?;

        let preview_size = self
            .vim()
            .preview_size(
                &self.context.env.provider_id,
                self.context.env.preview.winid,
            )
            .await?;

        let response = self.clone().process_query(cwd, query, preview_size, 1)?;

        self.vim()
            .call("clap#state#process_result_on_typed", response)
            .await?;

        Ok(())
    }

    async fn on_move(&mut self) -> Result<()> {
        let lnum = self.vim().display_getcurlnum().await?;

        let maybe_curline = self
            .lines
            .lock()
            .get(lnum - 1)
            .map(|r| r.item.raw_text().to_string());

        if let Some(curline) = maybe_curline {
            let preview_height = self.context.preview_height().await?;
            let preview_impl = PreviewImpl::new(curline, preview_height, &self.context)?;
            let preview = preview_impl.get_preview().await?;
            self.vim().render_preview(preview)?;
        }

        Ok(())
    }

    async fn on_typed(&mut self) -> Result<()> {
        let query = self.vim().input_get().await?;

        let response = tokio::task::spawn_blocking({
            let query = query.clone();
            let recent_files = self.clone();
            let cwd = self.context.cwd.clone();

            let lnum = self.vim().display_getcurlnum().await?;
            let preview_size = self
                .vim()
                .preview_size(
                    &self.context.env.provider_id,
                    self.context.env.preview.winid,
                )
                .await?;

            move || recent_files.process_query(cwd, query, preview_size, lnum)
        })
        .await??;

        let current_query = self.vim().input_get().await?;
        if current_query == query {
            self.vim()
                .call("clap#state#process_result_on_typed", response)
                .await?;
        }

        Ok(())
    }
}
