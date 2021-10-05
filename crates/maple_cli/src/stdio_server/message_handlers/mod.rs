//! Processes the RPC message directly.

use std::collections::HashMap;

use anyhow::Result;
use rayon::prelude::*;
use serde::Deserialize;
use serde_json::json;

use crate::previewer;
use crate::stdio_server::{MethodCall, write_response};

pub fn parse_filetypedetect(msg: MethodCall) {
    tokio::spawn(async move {
        let output = msg.get_string_unsafe("autocmd_filetypedetect");
        let ext_map: HashMap<&str, &str> = output
            .par_split(|x| x == '\n')
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
            .chain(
                vec![("h", "c"), ("hpp", "cpp"), ("vimrc", "vim"), ("cc", "cpp")].into_par_iter(),
            )
            .map(|(ext, ft)| (ext, ft))
            .collect();

        let method = "clap#ext#set";
        utility::println_json_with_length!(ext_map, method);
    });
}

async fn preview_file_impl(msg: MethodCall) -> Result<()> {
    let msg_id = msg.id;

    #[derive(Deserialize)]
    struct Params {
        fpath: String,
        display_width: usize,
        display_height: usize,
        preview_width: Option<usize>,
        preview_height: Option<usize>,
        preview_direction: String,
    }

    let Params {
        fpath,
        display_width,
        display_height,
        preview_width,
        preview_height,
        preview_direction,
    } = msg.parse()?;

    let fpath = crate::utils::expand_tilde(fpath)?;

    let (preview_height, preview_width) = if preview_direction.to_uppercase().as_str() == "UD" {
        (preview_height.unwrap_or(display_height), display_width)
    } else {
        (display_height, preview_width.unwrap_or(display_width))
    };

    let (lines, fname) = previewer::preview_file(fpath, preview_height, preview_width)?;

    let result = json!({"id": msg_id, "result": json!({"lines": lines, "fname": fname})});

    write_response(result);

    Ok(())
}

pub fn preview_file(msg: MethodCall) {
    tokio::spawn(async move {
        if let Err(e) = preview_file_impl(msg).await {
            log::error!("Error when previewing the file: {}", e);
        }
    });
}
