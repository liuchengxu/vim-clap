use crate::session::SessionContext;
use crate::types::Message;
use crate::write_response;
use serde_json::json;

pub mod on_move;
pub mod on_typed;

pub enum RpcMessage {
    OnMove(Message),
    OnTyped(Message),
}

pub trait HandleMessage: Send + 'static {
    fn handle(&self, msg: RpcMessage, context: &SessionContext);
}

#[derive(Clone)]
pub struct MessageHandler;

impl HandleMessage for MessageHandler {
    fn handle(&self, msg: RpcMessage, context: &SessionContext) {
        match msg {
            RpcMessage::OnMove(msg) => {
                let msg_id = msg.id;
                if let Err(e) = on_move::OnMoveHandler::try_new(msg, context).map(|x| x.handle()) {
                    write_response(json!({ "error": format!("{}",e), "id": msg_id }));
                }
            }
            RpcMessage::OnTyped(msg) => on_typed::handle_on_typed(msg, context),
        }
    }
}
