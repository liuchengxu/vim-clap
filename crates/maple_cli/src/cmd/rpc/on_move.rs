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

    let PreviewEnv { size, provider } = msg.try_into()?;

    let file_preview_impl = |path: &Path| {
        crate::utils::read_first_lines(path, 2 * size).map(|lines_iter| {
            let abs_path = canonicalize_and_as_str(path);
            (
                std::iter::once(abs_path.clone())
                    .chain(lines_iter)
                    .collect::<Vec<_>>(),
                abs_path,
            )
        })
    };

    match provider {
        Provider::Grep(preview_entry) => {
            match crate::utils::read_preview_lines(&preview_entry.fpath, preview_entry.lnum, size) {
                Ok((lines_iter, hi_lnum)) => {
                    let fname = format!("{}", preview_entry.fpath.display());
                    let lines = std::iter::once(fname.clone())
                        .chain(lines_iter)
                        .collect::<Vec<_>>();
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
                let lines = super::filer::read_dir_entries(&path, enable_icon, Some(2 * size))?;
                write_response(
                    json!({ "id": msg_id, "provider_id": "filer", "type": "preview", "lines": lines, "is_dir": true }),
                );
            } else {
                match file_preview_impl(&path) {
                    Ok((lines, abs_path)) => {
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
        Provider::Files(fpath) => match file_preview_impl(&fpath) {
            Ok((lines, abs_path)) => {
                write_response(
                    json!({ "id": msg_id, "provider_id": "files", "lines": lines, "fname": abs_path }),
                );
            }
            Err(err) => {
                error!(
                    "[files]Couldn't read first lines of {}, error: {:?}",
                    fpath.display(),
                    err
                );
            }
        },
    }

    Ok(())
}
