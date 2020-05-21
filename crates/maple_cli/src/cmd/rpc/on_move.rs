use super::types::{PreviewEnv, Provider};
use super::*;
use anyhow::Result;
use log::error;
use std::convert::TryInto;
use std::path::Path;

#[inline]
fn canonicalize_and_as_str<P: AsRef<Path>>(path: P) -> String {
    std::fs::canonicalize(path)
        .unwrap()
        .into_os_string()
        .into_string()
        .unwrap()
}

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
            match crate::utils::read_preview_lines(
                &preview_entry.fpath,
                preview_entry.lnum as usize,
                size as usize,
            ) {
                Ok((lines_iter, hi_lnum)) => {
                    let mut lines = lines_iter.collect::<Vec<_>>();
                    let fname = format!("{}", preview_entry.fpath.display());
                    lines.insert(0, fname.clone());
                    write_response(
                        json!({ "id": msg_id, "provider_id": "grep", "lines": lines, "fname": fname, "hi_lnum": hi_lnum }),
                    );
                }
                Err(err) => {
                    error!(
                        "[grep]Couldn't read first lines of {}, error: {:?}",
                        preview_entry.fpath.display(),
                        err
                    );
                }
            }
        }
        Provider::Filer { path, enable_icon } => {
            if path.is_dir() {
                let lines =
                    super::filer::read_dir_entries(&path, enable_icon, Some(2 * size as usize))?;
                write_response(
                    json!({ "id": msg_id, "provider_id": "filer", "type": "preview", "lines": lines, "is_dir": true }),
                );
            } else {
                match crate::utils::read_first_lines(&path, 10) {
                    Ok(line_iter) => {
                        let mut lines = line_iter.take(2 * size as usize).collect::<Vec<_>>();
                        let abs_path = canonicalize_and_as_str(&path);
                        lines.insert(0, abs_path.clone());
                        write_response(
                            json!({ "id": msg_id, "provider_id": "filer", "type": "preview", "lines": lines, "fname": abs_path }),
                        );
                    }
                    Err(err) => {
                        error!(
                            "[filer]Couldn't read first lines of {}, error: {:?}",
                            path.display(),
                            err
                        );
                    }
                }
            }
        }
        Provider::Files(fpath) => match crate::utils::read_first_lines(&fpath, 10) {
            Ok(line_iter) => {
                let mut lines = line_iter.collect::<Vec<_>>();
                let abs_path = canonicalize_and_as_str(&fpath);
                lines.insert(0, abs_path.clone());
                write_response(
                    json!({ "id": msg_id, "provider_id": "files", "lines": lines, "fname": abs_path }),
                );
            }
            Err(err) => {
                error!(
                    "Couldn't read first lines of {}, error: {:?}",
                    fpath.display(),
                    err
                );
            }
        },
    }

    Ok(())
}
