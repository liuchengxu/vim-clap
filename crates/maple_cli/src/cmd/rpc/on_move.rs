use super::types::{PreviewEnv, Provider};
use super::*;
use anyhow::Result;
use std::convert::TryInto;

pub(super) fn handle_message_on_move(msg: Message) -> Result<()> {
    let msg_id = msg.id;
    let PreviewEnv { size, provider } = match msg.try_into() {
        Ok(p) => p,
        Err(e) => {
            write_response(json!({ "error": format!("{}",e), "id": msg_id }));
            return Err(e);
        }
    };

    match provider {
        Provider::Grep(preview_entry) => {
            if let Ok((line_iter, hi_lnum)) = crate::utils::read_preview_lines(
                &preview_entry.fpath,
                preview_entry.lnum as usize,
                size as usize,
            ) {
                let mut lines = line_iter.collect::<Vec<_>>();
                let fname = format!("{}", preview_entry.fpath.display());
                lines.insert(0, fname.clone());
                write_response(
                    json!({ "lines": lines, "id": msg_id, "fname": fname, "hi_lnum": hi_lnum }),
                );
            }
        }
        Provider::Files(fpath) => {
            if let Ok(line_iter) = crate::utils::read_first_lines(&fpath, 10) {
                let mut lines = line_iter.collect::<Vec<_>>();
                let abs_path = std::fs::canonicalize(&fpath)
                    .unwrap()
                    .into_os_string()
                    .into_string()
                    .unwrap();
                lines.insert(0, abs_path.clone());
                write_response(json!({ "lines": lines, "id": msg_id, "fname": abs_path }));
            } else {
                write_response(json!({ "data": "Couldn't read_first_lines", "id": msg_id }));
            }
        }
    }

    Ok(())
}
