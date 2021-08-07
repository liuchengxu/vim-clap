//! Processes the RPC message wrapper in Event type.

pub mod on_init;
pub mod on_move;
pub mod on_typed;

use anyhow::Result;
use serde_json::json;

use crate::stdio_server::{
    session::{Event, EventHandler, SessionContext},
    write_response,
};

pub use on_move::{OnMove, OnMoveHandler};

#[derive(Clone)]
pub struct DefaultEventHandler;

#[async_trait::async_trait]
impl EventHandler for DefaultEventHandler {
    async fn handle(&mut self, event: Event, context: SessionContext) -> Result<()> {
        match event {
            Event::OnMove(msg) => {
                let msg_id = msg.id;
                if let Err(e) =
                    on_move::OnMoveHandler::try_new(&msg, &context, None).map(|x| x.handle())
                {
                    log::error!("Failed to handle OnMove event: {:?}", e);
                    write_response(json!({"error": e.to_string(), "id": msg_id }));
                }
            }
            Event::OnTyped(msg) => on_typed::handle_on_typed(msg, &context),
        }
        Ok(())
    }
}
