pub mod on_move;

use std::ops::Deref;
use std::sync::Arc;

use anyhow::Result;
use filter::{FilterContext, FilteredItem};
use parking_lot::Mutex;
use serde_json::json;

use crate::command::ctags::recursive::build_recursive_ctags_cmd;
use crate::command::grep::RgBaseCommand;
use crate::process::tokio::TokioCommand;
use crate::stdio_server::{
    session::{EventHandle, Scale, SessionContext, SyncFilterResults},
    write_response, MethodCall,
};

pub use on_move::{OnMove, OnMoveHandler};

#[derive(Clone)]
pub struct BuiltinHandle {
    pub current_results: Arc<Mutex<Vec<FilteredItem>>>,
}

impl BuiltinHandle {
    pub fn new() -> Self {
        Self {
            current_results: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl EventHandle for BuiltinHandle {
    async fn on_move(&mut self, msg: MethodCall, context: Arc<SessionContext>) -> Result<()> {
        let msg_id = msg.id;

        let scale = context.scale.lock();

        let curline = match (scale.deref(), msg.get_u64("lnum").ok()) {
            (Scale::Small { ref lines, .. }, Some(lnum)) => {
                if let Some(curline) = self
                    .current_results
                    .lock()
                    .get((lnum - 1) as usize)
                    .map(|r| r.source_item.raw.clone())
                {
                    Some(curline)
                } else {
                    lines.get(lnum as usize - 1).cloned()
                }
            }
            _ => None,
        };
        drop(scale);

        if let Err(error) =
            on_move::OnMoveHandler::create(&msg, &context, curline).map(|x| x.handle())
        {
            tracing::error!(?error, "Failed to handle OnMove event");
            write_response(json!({"error": error.to_string(), "id": msg_id }));
        }
        Ok(())
    }

    async fn on_typed(&mut self, msg: MethodCall, context: Arc<SessionContext>) -> Result<()> {
        let query = msg.get_query();

        let scale = context.scale.lock();

        match scale.deref() {
            Scale::Small { ref lines, .. } => {
                let SyncFilterResults {
                    total,
                    results,
                    decorated_lines:
                        printer::DecoratedLines {
                            lines,
                            indices,
                            truncated_map,
                        },
                } = context.sync_filter_source_item(&query, lines.iter().map(|s| s.as_str()))?;

                let method = "s:process_filter_message";
                utility::println_json_with_length!(total, lines, indices, truncated_map, method);

                let mut current_results = self.current_results.lock();
                *current_results = results;
            }
            Scale::Cache { ref path, .. } => {
                if let Err(e) = filter::dyn_run::<std::iter::Empty<_>>(
                    &query,
                    path.clone().into(),
                    FilterContext::new(
                        Default::default(),
                        context.icon,
                        Some(40),
                        Some(context.display_winwidth as usize),
                        context.match_type,
                    ),
                    context.match_bonuses.clone(),
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
pub async fn on_session_create(context: Arc<SessionContext>) -> Result<Scale> {
    let to_scale = |lines: Vec<String>| {
        let total = lines.len();

        if total > LARGE_SCALE {
            Scale::Large(total)
        } else {
            Scale::Small { total, lines }
        }
    };

    match context.provider_id.as_str() {
        "blines" => {
            let total =
                crate::utils::count_lines(std::fs::File::open(&context.start_buffer_path)?)?;
            return Ok(Scale::Cache {
                total,
                path: context.start_buffer_path.to_path_buf(),
            });
        }
        "tags" => {
            let lines = crate::command::ctags::buffer_tags_lines(&context.start_buffer_path)?;

            return Ok(Scale::Small {
                total: lines.len(),
                lines,
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
                    Some((total, path)) => Scale::Cache { total, path },
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
            let rg_cmd = RgBaseCommand::new(context.cwd.to_path_buf());
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
            return Ok(Scale::Cache { total, path });
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

    Ok(Scale::Indefinite)
}
