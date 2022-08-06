pub mod on_create;
pub mod on_move;

use std::ops::Deref;
use std::sync::Arc;

use anyhow::Result;
use filter::{matcher::Matcher, FilterContext, MatchedItem};
use filter::{ParSource, SourceItem};
use matcher::ClapItem;
use parking_lot::Mutex;
use serde_json::json;

use crate::command::ctags::recursive_tags::build_recursive_ctags_cmd;
use crate::command::grep::RgTokioCommand;
use crate::process::tokio::TokioCommand;
use crate::stdio_server::session::{EventHandle, SessionContext, SourceScale};
use crate::stdio_server::{write_response, MethodCall};

pub use on_move::{OnMove, OnMoveHandler};

#[derive(Clone)]
pub struct BuiltinHandle {
    pub current_results: Arc<Mutex<Vec<MatchedItem>>>,
}

impl BuiltinHandle {
    pub fn new() -> Self {
        Self {
            current_results: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// `lnum` is 1-based.
    fn line_at(&self, lnum: usize) -> Option<String> {
        self.current_results
            .lock()
            .get((lnum - 1) as usize)
            .map(|r| r.item.raw_text().to_string())
    }
}

#[async_trait::async_trait]
impl EventHandle for BuiltinHandle {
    async fn on_move(&mut self, msg: MethodCall, context: Arc<SessionContext>) -> Result<()> {
        let msg_id = msg.id;

        let curline = match (
            context.state.source_scale.lock().deref(),
            msg.get_u64("lnum").ok(),
        ) {
            (SourceScale::Small { ref items, .. }, Some(lnum)) => {
                if let Some(curline) = self.line_at(lnum as usize) {
                    Some(curline)
                } else {
                    items
                        .get(lnum as usize - 1)
                        .map(|item| item.raw_text().to_string())
                }
            }
            _ => None,
        };

        let on_move_handler = on_move::OnMoveHandler::create(&msg, &context, curline)?;
        if let Err(error) = on_move_handler.handle().await {
            tracing::error!(?error, "Failed to handle OnMove event");
            write_response(json!({"error": error.to_string(), "id": msg_id }));
        }
        Ok(())
    }

    async fn on_typed(&mut self, msg: MethodCall, context: Arc<SessionContext>) -> Result<()> {
        let query = msg.get_query();

        let source_scale = context.state.source_scale.lock();

        match source_scale.deref() {
            SourceScale::Small { ref items, .. } => {
                tracing::debug!("====================== [on_typed] Small");
                let matched_items =
                    filter::par_filter_items(query, items, &context.fuzzy_matcher());
                let matched = matched_items.len();
                // Take the first 200 entries and add an icon to each of them.
                printer::decorate_lines(
                    matched_items.iter().take(200).cloned().collect(),
                    context.display_winwidth as usize,
                    context.icon,
                )
                .print_on_typed(matched);
                let mut current_results = self.current_results.lock();
                *current_results = matched_items;
            }
            SourceScale::Cache { ref path, .. } => {
                if let Err(e) = filter::par_dyn_run(
                    &query,
                    FilterContext::new(
                        context.icon,
                        Some(40),
                        Some(context.display_winwidth as usize),
                        Matcher::default()
                            .set_match_scope(context.match_scope)
                            .set_bonuses(context.match_bonuses.clone()),
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
