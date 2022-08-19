mod on_create;
mod on_move;
mod providers;

use std::ops::Deref;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use serde_json::json;

use filter::{FilterContext, ParSource};
use matcher::Matcher;
use types::{ClapItem, MatchedItem, SourceItem};

use crate::command::ctags::recursive_tags::build_recursive_ctags_cmd;
use crate::command::grep::RgTokioCommand;
use crate::process::tokio::TokioCommand;
use crate::stdio_server::session::{ClapProvider, SessionContext, SourceScale};
use crate::stdio_server::{write_response, MethodCall};

pub use self::on_create::initialize;
pub use self::on_move::{OnMove, OnMoveHandler};
pub use self::providers::{dumb_jump, filer, recent_files};

#[derive(Debug)]
pub struct DefaultProvider {
    context: SessionContext,
    current_results: Arc<Mutex<Vec<MatchedItem>>>,
}

impl DefaultProvider {
    pub fn new(context: SessionContext) -> Self {
        Self {
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
    fn session_context(&self) -> &SessionContext {
        &self.context
    }

    async fn on_move(&mut self, msg: MethodCall) -> Result<()> {
        let msg_id = msg.id;

        let curline = match (
            self.context.state.source_scale.lock().deref(),
            msg.get_u64("lnum").ok(),
        ) {
            (SourceScale::Small { ref items, .. }, Some(lnum)) => {
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

        let on_move_handler = on_move::OnMoveHandler::create(&msg, &self.context, curline)?;
        if let Err(error) = on_move_handler.handle().await {
            tracing::error!(?error, "Failed to handle OnMove event");
            write_response(json!({"error": error.to_string(), "id": msg_id }));
        }
        Ok(())
    }

    async fn on_typed(&mut self, msg: MethodCall) -> Result<()> {
        let query = msg.get_query();

        let source_scale = self.context.state.source_scale.lock();

        match source_scale.deref() {
            SourceScale::Small { ref items, .. } => {
                let matched_items =
                    filter::par_filter_items(query, items, &self.context.fuzzy_matcher());
                let matched = matched_items.len();
                // Take the first 200 entries and add an icon to each of them.
                printer::decorate_lines(
                    matched_items.iter().take(200).cloned().collect(),
                    self.context.display_winwidth as usize,
                    self.context.icon,
                )
                .print_on_typed(matched);
                let mut current_results = self.current_results.lock();
                *current_results = matched_items;
            }
            SourceScale::Cache { ref path, .. } => {
                if let Err(e) = filter::par_dyn_run(
                    &query,
                    FilterContext::new(
                        self.context.icon,
                        Some(40),
                        Some(self.context.display_winwidth as usize),
                        Matcher::default()
                            .set_match_scope(self.context.match_scope)
                            .set_bonuses(self.context.match_bonuses.clone()),
                    ),
                    ParSource::File(path.clone()),
                ) {
                    tracing::error!(error = ?e, "Error occured when filtering the cache source");
                }
            }
            SourceScale::Large(_total) => {
                //TODO: probably remove this variant?
            }
            SourceScale::Indefinite => {
                // TODO: Note arbitrary shell command and use par_dyn_run later.
            }
        }

        Ok(())
    }
}
