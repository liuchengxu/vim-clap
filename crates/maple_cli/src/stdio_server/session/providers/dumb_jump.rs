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

pub async fn handle_dumb_jump_message(msg: Message) -> Vec<String> {
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
            let lines_clone = lines.clone();

            let total = lines.len();
            // Only show the top 200 items.
            lines.truncate(200);
            indices.truncate(200);
            let result = json!({
            "lines": lines,
            "indices": indices,
            "total": total,
            });

            let result = json!({ "id": msg_id, "provider_id": "dumb_jump", "result": result });
            write_response(result);

            return lines_clone;
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
    async fn handle(&mut self, event: Event, context: SessionContext) {
        match event {
            Event::OnMove(msg) => {
                let msg_id = msg.id;

                let lnum = msg.get_u64("lnum").expect("lnum exists");

                // lnum is 1-indexed
                if let Some(curline) = self.lines.get((lnum - 1) as usize) {
                    if let Err(e) = OnMoveHandler::with_curline(&msg, &context, curline.into())
                        .map(|x| x.handle())
                    {
                        log::error!("Failed to handle OnMove event: {:?}", e);
                        write_response(json!({"error": e.to_string(), "id": msg_id }));
                    }
                }
            }
            Event::OnTyped(msg) => {
                let lines = tokio::spawn(handle_dumb_jump_message(msg))
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
    }
}

pub struct DumbJumpSession;

impl NewSession for DumbJumpSession {
    fn spawn(msg: Message) -> Result<Sender<SessionEvent>> {
        let (session_sender, session_receiver) = crossbeam_channel::unbounded();

        let session = Session {
            session_id: msg.session_id,
            context: msg.into(),
            event_handler: DumbJumpMessageHandler::default(),
            event_recv: session_receiver,
        };

        session.start_event_loop()?;

        Ok(session_sender)
    }
}
