pub mod on_move;

use std::ops::Deref;
use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::Sender;
use serde_json::json;

use crate::process::tokio::TokioCommand;
use crate::stdio_server::{
    session::{
        EventHandler, NewSession, Scale, Session, SessionContext, SessionEvent, SyncFilterResults,
    },
    write_response, Message,
};

pub use on_move::{OnMove, OnMoveHandler};

pub struct BuiltinSession;

impl NewSession for BuiltinSession {
    fn spawn(msg: Message) -> Result<Sender<SessionEvent>> {
        let (session, session_sender) = Session::new(msg, BuiltinEventHandler);
        session.start_event_loop();
        Ok(session_sender)
    }
}

#[derive(Clone)]
pub struct BuiltinEventHandler;

#[async_trait::async_trait]
impl EventHandler for BuiltinEventHandler {
    async fn handle_on_move(&mut self, msg: Message, context: Arc<SessionContext>) -> Result<()> {
        let msg_id = msg.id;
        if let Err(e) = on_move::OnMoveHandler::create(&msg, &context, None).map(|x| x.handle()) {
            log::error!("Failed to handle OnMove event: {:?}", e);
            write_response(json!({"error": e.to_string(), "id": msg_id }));
        }
        Ok(())
    }

    async fn handle_on_typed(&mut self, msg: Message, context: Arc<SessionContext>) -> Result<()> {
        let query = msg.get_query();

        let scale = context.scale.lock();

        match scale.deref() {
            Scale::Small { ref lines, .. } => {
                let SyncFilterResults {
                    total,
                    decorated_lines:
                        printer::DecoratedLines {
                            lines,
                            indices,
                            truncated_map,
                        },
                } = context.sync_filter_source_item(&query, lines.iter().map(|s| s.as_str()))?;

                let method = "s:process_filter_message";
                utility::println_json_with_length!(total, lines, indices, truncated_map, method);
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
            let scale = if total > LARGE_SCALE {
                Scale::Large(total)
            } else {
                Scale::Small {
                    total,
                    lines: Vec::new(),
                }
            };
            return Ok(scale);
        }
        "proj_tags" => {
            let ctags_cmd = crate::command::ctags::recursive::build_recursive_ctags_cmd(
                context.cwd.to_path_buf(),
            );
            let lines = ctags_cmd.par_formatted_lines()?;
            return Ok(to_scale(lines));
        }
        "grep2" => {
            let rg_cmd = crate::command::grep::RgBaseCommand::new(context.cwd.to_path_buf());

            let send_response = |value: std::path::PathBuf| {
                let method = "clap#state#set_variable_string";
                let name = "g:__clap_forerunner_tempfile";
                utility::println_json_with_length!(name, value, method);
            };

            log::debug!("----------- grep2 cache info: {:?}", rg_cmd.cache_info());

            if let Some((total, path)) = rg_cmd.cache_info() {
                send_response(path.clone());
                return Ok(Scale::Cache { total, path });
            } else {
                let (total, path) = rg_cmd.create_cache().await?;
                send_response(path.clone());
                return Ok(Scale::Cache { total, path });
            }
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
