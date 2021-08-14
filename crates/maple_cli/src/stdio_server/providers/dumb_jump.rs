use anyhow::Result;
use crossbeam_channel::Sender;
use filter::Query;
use itertools::Itertools;
use log::error;
use serde::Deserialize;
use serde_json::json;

use crate::command::dumb_jump::{DumbJump, Lines};
use crate::stdio_server::event_handlers::OnMoveHandler;
use crate::stdio_server::{
    session::{Event, EventHandler, NewSession, Session, SessionContext, SessionEvent},
    write_response, Message,
};

pub async fn handle_dumb_jump_message(msg: Message, force_execute: bool) -> Vec<String> {
    let msg_id = msg.id;

    #[derive(Deserialize)]
    struct Params {
        cwd: String,
        query: String,
        extension: String,
    }

    let Params {
        cwd,
        query,
        extension,
    } = msg.deserialize_params_unsafe();

    if query.is_empty() {
        return Default::default();
    }

    let Query {
        exact_terms,
        inverse_terms,
        fuzzy_terms,
    } = Query::from(query.as_str());

    let parsed_query = fuzzy_terms.iter().map(|term| &term.word).join(" ");

    let dumb_jump = DumbJump {
        word: parsed_query,
        extension,
        kind: None,
        cmd_dir: Some(cwd.into()),
    };

    let result = match dumb_jump.references_or_occurrences(false).await {
        Ok(Lines { lines, mut indices }) => {
            let total_lines = lines
                .into_iter()
                .filter_map(|line| {
                    matcher::search_exact_terms(exact_terms.iter(), &line).map(|_| line)
                })
                .filter(|line| {
                    !inverse_terms
                        .iter()
                        .any(|term| term.matches_full_line(&line))
                })
                .collect::<Vec<_>>();

            let total = total_lines.len();
            // Only show the top 200 items.
            let lines = total_lines.iter().take(200).clone().collect::<Vec<_>>();
            indices.truncate(200);
            let result = json!({
            "lines": lines,
            "indices": indices,
            "total": total,
            });

            let result = json!({
              "id": msg_id,
              "force_execute": force_execute,
              "provider_id": "dumb_jump",
              "result": result,
            });

            write_response(result);

            return total_lines;
        }
        Err(e) => {
            error!("Error when running dumb_jump: {:?}", e);
            let error = json!({"message": e.to_string()});
            json!({ "id": msg_id, "provider_id": "dumb_jump", "error": error })
        }
    };

    write_response(result);

    Default::default()
}

#[derive(Debug, Clone, Default)]
pub struct DumbJumpMessageHandler {
    /// When passing the line content from Vim to Rust, for
    /// these lines that are extremely long, the performance
    /// of Vim can become very bad, we cache the display lines
    /// on Rust to pass the line number instead.
    lines: Vec<String>,
}

#[async_trait::async_trait]
impl EventHandler for DumbJumpMessageHandler {
    async fn handle(&mut self, event: Event, context: SessionContext) -> Result<()> {
        match event {
            Event::OnMove(msg) => {
                let msg_id = msg.id;

                let lnum = msg.get_u64("lnum").expect("lnum exists");

                // lnum is 1-indexed
                if let Some(curline) = self.lines.get((lnum - 1) as usize) {
                    if let Err(e) = OnMoveHandler::try_new(&msg, &context, Some(curline.into()))
                        .map(|x| x.handle())
                    {
                        log::error!("Failed to handle OnMove event: {:?}", e);
                        write_response(json!({"error": e.to_string(), "id": msg_id }));
                    }
                }
            }
            Event::OnTyped(msg) => {
                let lines = tokio::spawn(handle_dumb_jump_message(msg, false))
                    .await
                    .unwrap_or_else(|e| {
                        log::error!(
                            "Failed to spawn a task for handle_dumb_jump_message: {:?}",
                            e
                        );
                        Default::default()
                    });
                self.lines = lines;
            }
        }
        Ok(())
    }
}

pub struct DumbJumpSession;

impl NewSession for DumbJumpSession {
    fn spawn(msg: Message) -> Result<Sender<SessionEvent>> {
        let (session, session_sender) =
            Session::new(msg.clone(), DumbJumpMessageHandler::default());

        session.start_event_loop()?;

        tokio::spawn(async move {
            handle_dumb_jump_message(msg, true).await;
        });

        Ok(session_sender)
    }
}
