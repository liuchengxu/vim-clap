pub mod on_move;

use std::io::{BufRead, BufReader};
use std::ops::Deref;
use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::Sender;
use serde_json::json;

use crate::stdio_server::{
    session::{EventHandler, NewSession, Scale, Session, SessionContext, SessionEvent},
    write_response, Message,
};

pub use on_move::{OnMove, OnMoveHandler};

pub struct BuiltinSession;

impl NewSession for BuiltinSession {
    fn spawn(msg: Message) -> Result<Sender<SessionEvent>> {
        let (session, session_sender) = Session::new(msg, BuiltinEventHandler);
        session.start_event_loop()?;
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
                let ranked = filter::sync_run(
                    &query,
                    filter::Source::List(lines.iter().map(|s| s.as_str().into())), // TODO: optimize as_str().into(), clone happens there.
                    matcher::FuzzyAlgorithm::Fzy,
                    matcher::MatchType::TagName,
                    Vec::new(),
                )?;

                let total = ranked.len();

                // Take the first 200 entries and add an icon to each of them.
                let printer::DecoratedLines {
                    lines,
                    indices,
                    truncated_map,
                } = printer::decorate_lines(
                    ranked.iter().take(200).cloned().collect(),
                    context.display_winwidth as usize,
                    if context.enable_icon {
                        Some(icon::IconPainter::ProjTags)
                    } else {
                        None
                    },
                );

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

    if context.provider_id.as_str() == "blines" {
        let total = crate::utils::count_lines(std::fs::File::open(&context.start_buffer_path)?)?;
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

    if context.provider_id.as_str() == "proj_tags" {
        let ctags_cmd =
            crate::command::ctags::recursive::build_recursive_ctags_cmd(context.cwd.to_path_buf());
        let lines = ctags_cmd.formatted_tags_stream()?.collect::<Vec<_>>();
        return Ok(to_scale(lines));
    }

    if let Some(ref source_cmd) = context.source_cmd {
        // TODO: reuse the cache? in case of you run `fd --type f` under /
        let lines = BufReader::with_capacity(
            30 * 1024,
            filter::subprocess::Exec::shell(source_cmd)
                .cwd(&context.cwd)
                .stream_stdout()?,
        )
        .lines()
        .flatten()
        .collect::<Vec<_>>();

        return Ok(to_scale(lines));
    }

    Ok(Scale::Indefinite)
}
