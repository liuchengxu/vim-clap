pub mod on_move;

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
        // todo!()
        Ok(())
    }
}

/// Performs the initialization like collecting the source and total number of source items.
pub async fn on_session_create(context: Arc<SessionContext>) -> Result<Scale> {
    if context.provider_id.as_str() == "blines" {
        let total = crate::utils::count_lines(std::fs::File::open(&context.start_buffer_path)?)?;

        let scale = if total > 200_000 {
            Scale::Large(total)
        } else {
            Scale::Small {
                total,
                lines: Vec::new(),
            }
        };

        return Ok(scale);
    }

    Ok(Scale::Indefinite)
}
