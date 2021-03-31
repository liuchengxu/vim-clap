pub mod on_init;
pub mod on_move;
pub mod on_typed;

use std::collections::HashMap;

use serde_json::json;

use crate::stdio_server::{write_response, session::{HandleMessage, RpcMessage, SessionContext}, types::Message};

#[derive(Clone)]
pub struct MessageHandler;

impl HandleMessage for MessageHandler {
    fn handle(&self, msg: RpcMessage, context: &SessionContext) {
        match msg {
            RpcMessage::OnMove(msg) => {
                let msg_id = msg.id;
                if let Err(e) = on_move::OnMoveHandler::try_new(&msg, context).map(|x| x.handle()) {
                    log::error!("Handle RpcMessage::OnMove {:?}, error: {:?}", msg, e);
                    write_response(json!({"error": e.to_string(), "id": msg_id }));
                }
            }
            RpcMessage::OnTyped(msg) => on_typed::handle_on_typed(msg, context),
        }
    }
}

pub fn parse_filetypedetect(msg: Message) {
    let output = msg.get_string_unsafe("autocmd_filetypedetect");
    let ext_map: HashMap<String, String> = output
        .split('\n')
        .filter(|s| s.contains("setf"))
        .filter_map(|s| {
            // *.mkiv    setf context
            let items = s.split_whitespace().collect::<Vec<_>>();
            if items.len() != 3 {
                None
            } else {
                // (mkiv, context)
                items[0].split('.').last().map(|ext| (ext, items[2]))
            }
        })
        .chain(vec![("h", "c"), ("hpp", "cpp"), ("vimrc", "vim")].into_iter())
        .map(|(ext, ft)| (ext.into(), ft.into()))
        .collect();

    let result =
        json!({ "id": msg.id, "force_execute": true, "result": json!({"ext_map": ext_map}) });

    write_response(result);
}
