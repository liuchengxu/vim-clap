//! Processes the RPC message directly.

use std::collections::HashMap;

use anyhow::Result;
use serde_json::json;

use crate::stdio_server::{previewer, types::Message, write_response};

pub fn parse_filetypedetect(msg: Message) {
    let output = msg.get_string_unsafe("autocmd_filetypedetect");
    let ext_map: HashMap<&str, &str> = output
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
        .map(|(ext, ft)| (ext, ft))
        .collect();

    let result =
        json!({ "id": msg.id, "force_execute": true, "result": json!({"ext_map": ext_map}) });

    write_response(result);
}

async fn preview_file_impl(msg: Message) -> Result<()> {
    let fpath = msg.get_string("fpath")?;
    let winwidth = msg.get_u64("preview_width")?;
    let winheight = msg.get_u64("preview_height")?;

    let (lines, fname) = previewer::preview_file(fpath, winheight as usize, winwidth as usize)?;

    let result = json!({"id": msg.id, "result": json!({"lines": lines, "fname": fname})});

    write_response(result);

    Ok(())
}

pub fn preview_file(msg: Message) {
    tokio::spawn(async move {
        if let Err(e) = preview_file_impl(msg).await {
            log::error!("Error when previewing the file: {}", e);
        }
    });
}
