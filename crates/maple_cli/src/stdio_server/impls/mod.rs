mod on_create;
mod on_move;
mod providers;

use std::ops::Deref;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use serde_json::json;

use filter::{FilterContext, ParSource};
use printer::DisplayLines;
use types::MatchedItem;

use crate::stdio_server::session::{ClapProvider, SessionContext, SourceScale};

pub use self::on_create::initialize;
pub use self::on_move::{OnMoveHandler, PreviewKind};
pub use self::providers::{dumb_jump, filer, recent_files};

use super::vim::Vim;

#[derive(Debug)]
pub struct DefaultProvider {
    vim: Vim,
    context: SessionContext,
    current_results: Arc<Mutex<Vec<MatchedItem>>>,
}

impl DefaultProvider {
    pub fn new(vim: Vim, context: SessionContext) -> Self {
        Self {
            vim,
            context,
            current_results: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// `lnum` is 1-based.
    fn line_at(&self, lnum: usize) -> Option<String> {
        self.current_results
            .lock()
            .get((lnum - 1) as usize)
            .map(|r| r.item.output_text().to_string())
    }
}

#[async_trait::async_trait]
impl ClapProvider for DefaultProvider {
    fn vim(&self) -> &Vim {
        &self.vim
    }

    fn session_context(&self) -> &SessionContext {
        &self.context
    }

    async fn on_move(&mut self) -> Result<()> {
        let lnum = self.vim.display_getcurlnum().await?;

        let maybe_curline = match self.context.state.source_scale.lock().deref() {
            SourceScale::Small { ref items, .. } => {
                if let Some(curline) = self.line_at(lnum as usize) {
                    Some(curline)
                } else {
                    items
                        .get(lnum as usize - 1)
                        .map(|item| item.output_text().to_string())
                }
            }
            _ => None,
        };

        let curline = match maybe_curline {
            Some(line) => line,
            None => self.vim.display_getcurline().await?,
        };

        if curline.is_empty() {
            tracing::debug!("[DefaultProvider::on_move] curline is empty, skipping on_move");
            return Ok(());
        }

        let on_move_handler = on_move::OnMoveHandler::create(curline, &self.context)?;

        // TODO: preview cache
        let preview_result = on_move_handler.get_preview().await?;

        self.vim
            .exec("clap#state#process_preview_result", preview_result)?;

        Ok(())
    }

    async fn on_typed(&mut self) -> Result<()> {
        let query = self.vim.input_get().await?;

        let source_scale = self.context.state.source_scale.lock();

        match source_scale.deref() {
            SourceScale::Small { ref items, .. } => {
                let matched_items = filter::par_filter_items(query, items, &self.context.matcher);
                // Take the first 200 entries and add an icon to each of them.
                let DisplayLines {
                    lines,
                    indices,
                    truncated_map,
                    icon_added,
                } = printer::decorate_lines(
                    matched_items.iter().take(200).cloned().collect(),
                    self.context.display_winwidth as usize,
                    self.context.icon,
                );
                let msg = json!({
                    "total": matched_items.len(),
                    "lines": lines,
                    "indices": indices,
                    "icon_added": icon_added,
                    "truncated_map": truncated_map,
                });
                self.vim()
                    .exec("clap#state#process_filter_message", json!([msg, true]))?;
                let mut current_results = self.current_results.lock();
                *current_results = matched_items;
            }
            SourceScale::Cache { ref path, .. } => {
                // TODO: Watcher::Rpc, Watcher::Println
                if let Err(e) = filter::par_dyn_run(
                    &query,
                    FilterContext::new(
                        self.context.icon,
                        Some(40),
                        Some(self.context.display_winwidth as usize),
                        self.context.matcher.clone(),
                    ),
                    ParSource::File(path.clone()),
                ) {
                    tracing::error!(error = ?e, "Error occured when filtering the cache source");
                }
            }
            SourceScale::Large(_total) => {
                //TODO: probably remove this variant?
            }
            SourceScale::Unknown => {
                // TODO: Note arbitrary shell command and use par_dyn_run later.
            }
        }

        Ok(())
    }
}
