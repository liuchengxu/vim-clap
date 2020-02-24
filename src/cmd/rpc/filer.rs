use std::{fs, io};

use anyhow::Result;
use serde_json::json;

use super::{write_response, Message};
use crate::icon::prepend_filer_icon;

fn into_string(entry: std::fs::DirEntry, enable_icon: bool) -> String {
    let path_str = if entry.path().is_dir() {
        format!(
            "{}/",
            entry
                .path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap()
        )
    } else {
        entry
            .path()
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .map(Into::into)
            .unwrap()
    };

    if enable_icon {
        prepend_filer_icon(&entry.path(), &path_str)
    } else {
        path_str
    }
}

fn read_dir_entries(dir: &str, enable_icon: bool) -> Result<Vec<String>> {
    let mut entries = fs::read_dir(dir)?
        .map(|res| res.map(|x| into_string(x, enable_icon)))
        .collect::<Result<Vec<_>, io::Error>>()?;

    entries.sort();

    Ok(entries)
}

pub(super) fn handle_message(msg: Message) {
    if let Some(dir) = msg.params.get("cwd").and_then(|x| x.as_str()) {
        let enable_icon = msg
            .params
            .get("enable_icon")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        let result = match read_dir_entries(&dir, enable_icon) {
            Ok(entries) => {
                let result = json!({
                "entries": entries,
                "dir": dir,
                "total": entries.len(),
                });
                json!({ "result": result, "id": msg.id })
            }
            Err(err) => {
                let error = json!({"message": format!("{}", err), "dir": dir});
                json!({ "error": error, "id": msg.id })
            }
        };
        write_response(result);
    }
}

#[test]
fn test_dir() {
    let entries = read_dir_entries("/.DocumentRevisions-V100/", true).unwrap();
    println!("entry: {:?}", entries);
}
