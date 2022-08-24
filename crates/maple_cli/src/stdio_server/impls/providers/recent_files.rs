use matcher::ClapItem;
use parking_lot::Mutex;
use std::sync::Arc;

use anyhow::Result;
use serde_json::{json, Value};

use types::{MatchedItem, Score};

use crate::datastore::RECENT_FILES_IN_MEMORY;
use crate::stdio_server::impls::OnMoveHandler;
use crate::stdio_server::session::{ClapProvider, SessionContext};
use crate::stdio_server::vim::Vim;
use crate::stdio_server::MethodCall;

#[derive(Debug, Clone)]
pub struct RecentFilesProvider {
    vim: Vim,
    context: Arc<SessionContext>,
    lines: Arc<Mutex<Vec<MatchedItem>>>,
}

impl RecentFilesProvider {
    pub fn new(vim: Vim, context: SessionContext) -> Self {
        Self {
            vim,
            context: Arc::new(context),
            lines: Default::default(),
        }
    }

    fn process_query(
        self,
        cwd: String,
        query: String,
        enable_icon: bool,
        lnum: u64,
    ) -> Result<Value> {
        let mut recent_files = RECENT_FILES_IN_MEMORY.lock();

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

        let winwidth = self.context.display_winwidth as usize;

        // process the new preview
        let preview = if let Some(new_entry) = ranked.get(lnum as usize - 1) {
            let new_curline = new_entry.display_text().to_string();
            if let Ok((lines, fname)) = crate::previewer::preview_file(
                new_curline,
                self.context.sensible_preview_size(),
                winwidth,
            ) {
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
            if enable_icon {
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
    fn session_context(&self) -> &SessionContext {
        &self.context
    }

    async fn on_create(&mut self) -> Result<()> {
        let query = self.vim.context_query_or_input().await?;
        let cwd = self.vim.working_dir().await?;
        let enable_icon = self.vim.get_var_bool("clap_enable_icon").await?;

        let response = self.clone().process_query(cwd, query, enable_icon, 1)?;

        self.vim
            .call("clap#state#process_result_on_typed", response)
            .await?;

        Ok(())
    }

    async fn on_move(&mut self) -> Result<()> {
        let lnum = self.vim.display_getcurlnum().await?;

        let maybe_curline = self
            .lines
            .lock()
            .get((lnum - 1) as usize)
            .map(|r| r.item.raw_text().to_string());

        if let Some(curline) = maybe_curline {
            let on_move_handler = OnMoveHandler::create(curline, &self.context)?;
            let preview_result = on_move_handler.on_move_process().await?;
            self.vim
                .exec("clap#state#process_preview_result", preview_result)?;
        }

        Ok(())
    }

    async fn on_typed(&mut self, _msg: MethodCall) -> Result<()> {
        let query = self.vim.input_get().await?;
        let cwd = self.vim.working_dir().await?;
        let lnum = self.vim.display_getcurlnum().await?;
        let enable_icon = self.vim.get_var_bool("clap_enable_icon").await?;

        let recent_files = self.clone();
        let response = tokio::task::spawn_blocking(move || {
            recent_files.process_query(cwd, query, enable_icon, lnum)
        })
        .await??;

        self.vim
            .call("clap#state#process_result_on_typed", response)
            .await?;

        Ok(())
    }
}
