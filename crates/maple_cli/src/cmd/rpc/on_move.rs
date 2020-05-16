use super::types::Provider;
use super::*;
use anyhow::Result;
use std::convert::TryInto;

pub(super) fn handle_message_on_move(msg: Message) -> Result<()> {
    let msg_id = msg.id;
    let provider: Provider = msg.try_into()?;

    match provider {
        Provider::Grep(preview_entry) => {
            if let Ok((line_iter, hl_line)) = crate::utils::read_preview_lines(
                &preview_entry.fpath,
                preview_entry.lnum as usize,
                5,
            ) {
                let mut lines = line_iter.collect::<Vec<_>>();
                lines.insert(0, format!("{}", preview_entry.fpath.display()));
                write_response(
                    json!({ "lines": lines, "id": msg_id, "fname": format!("{}", preview_entry.fpath.display()), "hi_lnum": hl_line }),
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
        _ => write_response(
            json!({ "error": format!("Unknown provider_id: {}", "unknonw provider id"), "id": msg_id }),
        ),
    }

    Ok(())
}
