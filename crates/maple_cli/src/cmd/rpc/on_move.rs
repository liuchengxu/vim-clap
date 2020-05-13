use super::*;

pub(super) fn handle_message_on_move(msg: Message) {
    let cwd = String::from(
        msg.params
            .get("cwd")
            .and_then(|x| x.as_str())
            .unwrap_or("Missing cwd when deserializing into FilerParams"),
    );

    let mut fpath: std::path::PathBuf = cwd.into();

    let fname = String::from(
        msg.params
            .get("curline")
            .and_then(|x| x.as_str())
            .unwrap_or("Missing fname when deserializing into FilerParams"),
    );

    fpath.push(fname);

    let enable_icon = msg
        .params
        .get("enable_icon")
        .and_then(|x| x.as_bool())
        .unwrap_or(false);

    let provider_id = msg
        .params
        .get("provider_id")
        .and_then(|x| x.as_str())
        .unwrap_or("Unknown provider id");

    match provider_id {
        "grep" => {
            lazy_static::lazy_static! {
                static ref GREP_RE: regex::Regex = regex::Regex::new(r"^(.*):\d+:\d+:").unwrap();
            }
        }
        "files" => {
            if let Ok(line_iter) = crate::utils::read_first_lines(&fpath, 10) {
                let mut lines = line_iter.collect::<Vec<_>>();
                let abs_path = std::fs::canonicalize(&fpath)
                    .unwrap()
                    .into_os_string()
                    .into_string()
                    .unwrap();
                lines.insert(0, abs_path.clone());
                write_response(json!({ "lines": lines, "id": msg.id, "fname": abs_path }));
            } else {
                write_response(
                    json!({ "data": serde_json::to_string(&msg).unwrap(), "id": msg.id }),
                );
            }
        }
        _ => write_response(
            json!({ "error": format!("Unknown provider_id: {}", provider_id), "id": msg.id }),
        ),
    }
}
