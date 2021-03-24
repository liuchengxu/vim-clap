use std::collections::HashMap;

use serde_json::json;

use crate::stdio_server::{types::Message, write_response};

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
