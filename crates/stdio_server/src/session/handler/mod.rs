use crate::session::SessionContext;
use crate::types::Message;
use crate::write_response;
use serde_json::json;

pub mod on_move;
pub mod on_typed;

pub enum Handler {
    OnMove,
    OnTyped,
}

impl Handler {
    pub fn execute(self, msg: Message, context: &SessionContext) {
        let msg_id = msg.id;
        match self {
            Self::OnMove => {
                if let Err(e) = on_move::OnMoveHandler::try_new(msg, context).map(|x| x.handle()) {
                    write_response(json!({ "error": format!("{}",e), "id": msg_id }));
                }
            }
            Self::OnTyped => on_typed::handle_on_typed(msg, context),
        }
    }
}
