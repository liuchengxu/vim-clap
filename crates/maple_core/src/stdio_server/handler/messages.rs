use crate::datastore::RECENT_FILES_IN_MEMORY;
use anyhow::{anyhow, Result};
use rpc::MethodCall;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn note_recent_file(file_path: String) -> Result<()> {
    tracing::debug!(?file_path, "Received a recent file notification");

    let path = std::path::Path::new(&file_path);
    if !path.exists() || !path.is_file() {
        return Ok(());
    }

    let mut recent_files = RECENT_FILES_IN_MEMORY.lock();
    recent_files.upsert(file_path);

    Ok(())
}

pub async fn preview_file(msg: MethodCall) -> Result<Value> {
    let msg_id = msg.id;

    #[derive(Deserialize)]
    struct InnerParams {
        fpath: String,
        display_width: usize,
        display_height: usize,
        preview_width: Option<usize>,
        preview_height: Option<usize>,
        preview_direction: String,
    }

    let InnerParams {
        fpath,
        display_width,
        display_height,
        preview_width,
        preview_height,
        preview_direction,
    } = msg.params.parse()?;

    let fpath = crate::paths::expand_tilde(fpath);

    let (preview_height, preview_width) = if preview_direction.to_uppercase().as_str() == "UD" {
        (preview_height.unwrap_or(display_height), display_width)
    } else {
        (display_height, preview_width.unwrap_or(display_width))
    };

    let (lines, fname) = crate::previewer::preview_file(fpath, preview_height, preview_width)?;

    let value = json!({"id": msg_id, "result": json!({"lines": lines, "fname": fname})});

    Ok(value)
}

pub async fn preview_quickfix(msg: MethodCall) -> Result<Value> {
    use crate::previewer::{preview_file, preview_file_at};
    use std::path::PathBuf;

    let msg_id = msg.id;

    #[derive(Deserialize)]
    struct InnerParams {
        cwd: String,
        curline: String,
        winwidth: usize,
        winheight: usize,
    }

    let InnerParams {
        cwd,
        curline,
        winwidth,
        winheight,
    } = msg.params.parse()?;

    let (p, lnum) = parse_quickfix_entry(curline.as_str())?;

    let mut fpath: PathBuf = cwd.into();
    fpath.push(p);

    let result = if lnum == 0 {
        let size = winheight + 5;
        let (lines, _) = preview_file(fpath.as_path(), size, winwidth)?;
        json!({ "event": "on_move", "lines": lines, "fname": fpath })
    } else {
        let (lines, hi_lnum) = preview_file_at(fpath.as_path(), winheight, winwidth, lnum)?;
        json!({ "event": "on_move", "lines": lines, "fname": fpath, "hi_lnum": hi_lnum })
    };

    let value = json!({ "id": msg_id, "provider_id": "quickfix", "result": result });

    Ok(value)
}

fn parse_quickfix_entry(line: &str) -> Result<(&str, usize)> {
    let mut splitted = line.split('|');
    let fpath = splitted
        .next()
        .ok_or_else(|| anyhow!("Can not find fpath in {}", line))?;

    let mut it = splitted
        .next()
        .ok_or_else(|| anyhow!("Can not find lnum and column info in {}", line))?
        .split("col");

    let lnum = it
        .next()
        .ok_or_else(|| anyhow!("Can not find lnum in {}", line))?
        .trim()
        .parse::<usize>()?;

    Ok((fpath, lnum))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quickdix_display_line_works() {
        let line = "test/bench/python/test_fuzzy_filter.vim|0 col 0| Modified 2月,13 2021 10:58:27 rw-rw-r--";
        assert_eq!(
            parse_quickfix_entry(line).unwrap(),
            ("test/bench/python/test_fuzzy_filter.vim", 0usize)
        );
    }
}
