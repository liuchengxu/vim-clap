use std::path::PathBuf;

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::json;

use crate::previewer::{preview_file, preview_file_at};
use crate::stdio_server::{write_response, MethodCall};

pub fn preview_quickfix_entry(msg: MethodCall) {
    tokio::spawn(async move { preview_quickfix_entry_impl(msg).await });
}

async fn preview_quickfix_entry_impl(msg: MethodCall) -> Result<()> {
    let msg_id = msg.id;

    #[derive(Deserialize)]
    struct Params {
        cwd: String,
        curline: String,
        winwidth: u64,
        winheight: u64,
    }

    let Params {
        cwd,
        curline,
        winwidth,
        winheight,
    } = msg.parse()?;

    let (p, lnum) = parse_quickfix_entry(curline.as_str())?;

    let mut fpath: PathBuf = cwd.into();
    fpath.push(p);

    let result = if lnum == 0 {
        let size = (winheight + 5) as usize;
        let (lines, _) = preview_file(fpath.as_path(), size, winwidth as usize)?;
        json!({ "event": "on_move", "lines": lines, "fname": fpath })
    } else {
        let size = (winheight / 2) as usize;
        let (lines, hi_lnum) = preview_file_at(fpath.as_path(), size, winwidth as usize, lnum)?;
        json!({ "event": "on_move", "lines": lines, "fname": fpath, "hi_lnum": hi_lnum })
    };

    write_response(json!({ "id": msg_id, "provider_id": "quickfix", "result": result }));

    Ok(())
}

pub(crate) fn parse_quickfix_entry(line: &str) -> Result<(&str, usize)> {
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
            ("test/bench/python/test_fuzzy_filter.vim".into(), 0usize)
        );
    }
}
