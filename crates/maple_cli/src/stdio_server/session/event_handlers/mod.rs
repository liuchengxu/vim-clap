//! Processes the RPC message wrapper in Event type.

pub mod on_init;
pub mod on_move;
pub mod on_typed;

use serde_json::json;

use crate::stdio_server::{
    session::{Event, EventHandler, SessionContext},
    write_response,
};

#[derive(Clone)]
pub struct DefaultEventHandler;

impl EventHandler for DefaultEventHandler {
    fn handle(&self, event: Event, context: &SessionContext) {
        match event {
            Event::OnMove(msg) => {
                let msg_id = msg.id;
                if let Err(e) = on_move::OnMoveHandler::try_new(&msg, context).map(|x| x.handle()) {
                    log::error!("Handle Event::OnMove {:?}, error: {:?}", msg, e);
                    write_response(json!({"error": e.to_string(), "id": msg_id }));
                }
            }
            Event::OnTyped(msg) => on_typed::handle_on_typed(msg, context),
        }
    }
}
