use log::{debug, error};
use serde_json::json;

use crate::cmd::dumb_jump::{DumbJump, Lines};
use crate::stdio_server::{write_response, Message};

pub fn handle_dumb_jump_message(msg: Message) {
    tokio::spawn(async move {
        let cwd = msg.get_cwd();
        let input = msg.get_string_unsafe("input");
        let extension = msg.get_string_unsafe("extension");
        debug!("==> Recv dumb_jump params: cwd:{}", cwd);

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

                json!({ "id": msg.id, "provider_id": "dumb_jump", "result": result })
            }
            Err(e) => {
                error!("error when running dumb_jump: {:?}", e);
                let error = json!({"message": e.to_string()});
                json!({ "id": msg.id, "provider_id": "dumb_jump", "error": error })
            }
        };

        write_response(result);
    });
}
