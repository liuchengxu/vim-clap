use anyhow::Result;
use crossbeam_channel::Sender;
use log::{debug, error};
use serde_json::json;

use crate::commands::dumb_jump::{DumbJump, Lines};
use crate::stdio_server::{
    session::{
        build_abs_path, Event, EventHandler, NewSession, OnMove, OnMoveHandler, Session,
        SessionContext, SessionEvent,
    },
    write_response, Message,
};

pub async fn handle_dumb_jump_message(msg: Message) {
    log::debug!(
        "----------------- [handle_dumb_jump_message] id: {}",
        msg.id,
    );

    debug!(
        "----------------- Handle the dumb jump message, id: {}",
        msg.id,
    );
    // tokio::spawn(async move {
    debug!("----------- moved msg: {:?}", msg);
    let cwd = msg.get_cwd();
    let input = msg.get_string_unsafe("input");
    let extension = msg.get_string_unsafe("extension");

    let dumb_jump = DumbJump {
        word: input,
        extension,
        kind: None,
        cmd_dir: Some(cwd.into()),
    };

    let msg_id = msg.id;
    let result = match dumb_jump.references_or_occurrences(false).await {
        Ok(Lines {
            mut lines,
            mut indices,
        }) => {
            let total = lines.len();
            // Only show the top 200 items.
            lines.truncate(200);
            indices.truncate(200);
            let result = json!({
            "lines": lines,
            "indices": indices,
            "total": total,
            });

            json!({ "id": msg_id, "provider_id": "dumb_jump", "result": result })
        }
        Err(e) => {
            error!("error when running dumb_jump: {:?}", e);
            let error = json!({"message": e.to_string()});
            json!({ "id": msg_id, "provider_id": "dumb_jump", "error": error })
        }
    };

    debug!("sending result, id: {:?}", msg_id);
    write_response(result);
    // });
}

#[derive(Clone)]
pub struct DumbJumpMessageHandler;

#[async_trait::async_trait]
impl EventHandler for DumbJumpMessageHandler {
    async fn handle(&self, event: Event, context: SessionContext) {
        match event {
            Event::OnMove(msg) => {
                let msg_id = msg.id;
                if let Err(e) =
                    super::event_handlers::on_move::OnMoveHandler::try_new(&msg, &context)
                        .map(|x| x.handle())
                {
                    log::error!("Failed to handle OnMove event: {:?}", e);
                    write_response(json!({"error": e.to_string(), "id": msg_id }));
                }
            }
            Event::OnTyped(msg) => {
                log::debug!("handling msg id: {}", msg.id);
                tokio::spawn(async {
                    handle_dumb_jump_message(msg).await;
                })
                .await;
            }
        }
    }
}

pub struct DumbJumpSession;

impl NewSession for DumbJumpSession {
    fn spawn(&self, msg: Message) -> Result<Sender<SessionEvent>> {
        let (session_sender, session_receiver) = crossbeam_channel::unbounded();

        let session = Session {
            session_id: msg.session_id,
            context: msg.clone().into(),
            event_handler: DumbJumpMessageHandler,
            event_recv: session_receiver,
        };

        log::debug!(
            "----------------- created new dumb jump session, msg.id: {}",
            msg.id
        );
        // handle_dumb_jump_message(msg);

        session.start_event_loop()?;

        Ok(session_sender)
    }
}
