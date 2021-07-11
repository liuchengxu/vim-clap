use anyhow::Result;
use crossbeam_channel::Sender;
use log::error;
use serde::Deserialize;
use serde_json::json;

use crate::commands::dumb_jump::{DumbJump, Lines};
use crate::stdio_server::{
    session::{
        Event, EventHandler, NewSession, OnMoveHandler, Session, SessionContext, SessionEvent,
    },
    write_response, Message,
};

pub async fn handle_dumb_jump_message(msg: Message) {
    let msg_id = msg.id;

    #[derive(Deserialize)]
    struct Params {
        cwd: String,
        input: String,
        extension: String,
    }

    let Params {
        cwd,
        input,
        extension,
    } = msg
        .deserialize_params()
        .unwrap_or_else(|e| panic!("Failed to deserialize dumb_jump params: {:?}", e));

    let dumb_jump = DumbJump {
        word: input,
        extension,
        kind: None,
        cmd_dir: Some(cwd.into()),
    };

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
            error!("Error when running dumb_jump: {:?}", e);
            let error = json!({"message": e.to_string()});
            json!({ "id": msg_id, "provider_id": "dumb_jump", "error": error })
        }
    };

    write_response(result);
}

#[derive(Debug, Clone)]
pub struct DumbJumpMessageHandler;

#[async_trait::async_trait]
impl EventHandler for DumbJumpMessageHandler {
    async fn handle(&self, event: Event, context: SessionContext) {
        match event {
            Event::OnMove(msg) => {
                let msg_id = msg.id;
                if let Err(e) = OnMoveHandler::try_new(&msg, &context).map(|x| x.handle()) {
                    log::error!("Failed to handle OnMove event: {:?}", e);
                    write_response(json!({"error": e.to_string(), "id": msg_id }));
                }
            }
            Event::OnTyped(msg) => {
                tokio::spawn(async {
                    handle_dumb_jump_message(msg).await;
                })
                .await
                .unwrap_or_else(|e| {
                    log::error!(
                        "Failed to spawn a task for handle_dumb_jump_message: {:?}",
                        e
                    );
                });
            }
        }
    }
}

pub struct DumbJumpSession;

impl NewSession for DumbJumpSession {
    fn spawn(msg: Message) -> Result<Sender<SessionEvent>> {
        let (session_sender, session_receiver) = crossbeam_channel::unbounded();

        let session = Session {
            session_id: msg.session_id,
            context: msg.into(),
            event_handler: DumbJumpMessageHandler,
            event_recv: session_receiver,
        };

        // handle_dumb_jump_message(msg);

        session.start_event_loop()?;

        Ok(session_sender)
    }
}
