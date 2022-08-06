pub mod on_move;

use std::ops::Deref;
use std::sync::Arc;

use anyhow::Result;
use filter::SourceItem;
use filter::{matcher::Matcher, FilterContext, MatchedItem};
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
                let results = filter::par_filter_items(query, items, &context.fuzzy_matcher());
                let total = results.len();
                // Take the first 200 entries and add an icon to each of them.
                printer::decorate_lines(
                    results.iter().take(200).cloned().collect(),
                    context.display_winwidth as usize,
                    context.icon,
                )
                .print_on_typed(total);

                let mut current_results = self.current_results.lock();
                *current_results = results;
            }
            SourceScale::Cache { ref path, .. } => {
                if let Err(e) = filter::dyn_run::<std::iter::Empty<_>>(
                    &query,
                    FilterContext::new(
                        context.icon,
                        Some(40),
                        Some(context.display_winwidth as usize),
                        Matcher::default()
                            .set_match_scope(context.match_scope)
                            .set_bonuses(context.match_bonuses.clone()),
                    ),
                    path.clone().into(),
                ) {
                    tracing::error!(error = ?e, "Error occured when filtering the cache source");
                }
            }
            _ => {}
        }

        Ok(())
    }
}

/// Threshold for large scale.
const LARGE_SCALE: usize = 200_000;

/// Performs the initialization like collecting the source and total number of source items.
pub async fn on_session_create(context: Arc<SessionContext>) -> Result<SourceScale> {
    let to_scale = |lines: Vec<String>| {
        let total = lines.len();

        if total > LARGE_SCALE {
            SourceScale::Large(total)
        } else {
            let items = lines
                .into_iter()
                .map(|line| Arc::new(SourceItem::from(line)) as Arc<dyn ClapItem>)
                .collect::<Vec<_>>();
            SourceScale::Small { total, items }
        }
    };

    match context.provider_id.as_str() {
        "blines" => {
            let total =
                crate::utils::count_lines(std::fs::File::open(&context.start_buffer_path)?)?;
            return Ok(SourceScale::Cache {
                total,
                path: context.start_buffer_path.to_path_buf(),
            });
        }
        "tags" => {
            let items = crate::command::ctags::buffer_tags::buffer_tag_items(
                &context.start_buffer_path,
                false,
            )?;
            return Ok(SourceScale::Small {
                total: items.len(),
                items,
            });
        }
        "proj_tags" => {
            let ctags_cmd = build_recursive_ctags_cmd(context.cwd.to_path_buf());
            let scale = if context.no_cache {
                let lines = ctags_cmd.par_formatted_lines()?;
                ctags_cmd.create_cache_async(lines.clone()).await?;
                to_scale(lines)
            } else {
                match ctags_cmd.ctags_cache() {
                    Some((total, path)) => SourceScale::Cache { total, path },
                    None => {
                        let lines = ctags_cmd.par_formatted_lines()?;
                        ctags_cmd.create_cache_async(lines.clone()).await?;
                        to_scale(lines)
                    }
                }
            };
            return Ok(scale);
        }
        "grep2" => {
            let rg_cmd = RgTokioCommand::new(context.cwd.to_path_buf());
            let (total, path) = if context.no_cache {
                rg_cmd.create_cache().await?
            } else {
                match rg_cmd.cache_info() {
                    Some(cache) => cache,
                    None => rg_cmd.create_cache().await?,
                }
            };
            let method = "clap#state#set_variable_string";
            let name = "g:__clap_forerunner_tempfile";
            let value = &path;
            utility::println_json_with_length!(method, name, value);
            return Ok(SourceScale::Cache { total, path });
        }
        _ => {}
    }

    if let Some(ref source_cmd) = context.source_cmd {
        // TODO: check cache

        // Can not use subprocess::Exec::shell here.
        //
        // Must use TokioCommand otherwise the timeout may not work.
        let lines = TokioCommand::new(source_cmd)
            .current_dir(&context.cwd)
            .lines()
            .await?;

        return Ok(to_scale(lines));
    }

    Ok(SourceScale::Indefinite)
}
