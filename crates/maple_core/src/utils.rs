use anyhow::Result;
use chrono::prelude::*;
use icon::Icon;
use std::io::{BufRead, Lines};
use std::path::Path;
use subprocess::Exec;
use utils::{println_json, println_json_with_length, read_first_lines};

pub type UtcTime = DateTime<Utc>;

#[derive(Debug, Clone)]
#[allow(unused)]
pub enum SendResponse {
    Json,
    JsonWithContentLength,
}

/// Reads the first lines from cache file and send back the cached info.
pub fn send_response_from_cache(
    tempfile: &Path,
    total: usize,
    response_ty: SendResponse,
    icon: Icon,
) {
    let using_cache = true;
    if let Ok(iter) = read_first_lines(&tempfile, 100) {
        let lines: Vec<String> = if let Some(icon_kind) = icon.icon_kind() {
            iter.map(|x| icon_kind.add_icon_to_text(x)).collect()
        } else {
            iter.collect()
        };
        match response_ty {
            SendResponse::Json => println_json!(total, tempfile, using_cache, lines),
            SendResponse::JsonWithContentLength => {
                println_json_with_length!(total, tempfile, using_cache, lines)
            }
        }
    } else {
        match response_ty {
            SendResponse::Json => println_json!(total, tempfile, using_cache),
            SendResponse::JsonWithContentLength => {
                println_json_with_length!(total, tempfile, using_cache)
            }
        }
    }
}

/// Counts lines in the source `handle`.
///
/// # Examples
/// ```ignore
/// let lines: usize = count_lines(std::fs::File::open("Cargo.toml").unwrap()).unwrap();
/// ```
///
/// Credit: https://github.com/eclarke/linecount/blob/master/src/lib.rs
pub fn count_lines<R: std::io::Read>(handle: R) -> std::io::Result<usize> {
    let mut reader = std::io::BufReader::with_capacity(1024 * 32, handle);
    let mut count = 0;
    loop {
        let len = {
            let buf = reader.fill_buf()?;
            if buf.is_empty() {
                break;
            }
            count += bytecount::count(buf, b'\n');
            buf.len()
        };
        reader.consume(len);
    }

    Ok(count)
}

#[inline]
pub fn lines(cmd: Exec) -> Result<Lines<impl BufRead>> {
    // We usually have a decent amount of RAM nowdays.
    Ok(std::io::BufReader::with_capacity(8 * 1024 * 1024, cmd.stream_stdout()?).lines())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_lines() {
        let f: &[u8] = b"some text\nwith\nfour\nlines\n";
        assert_eq!(count_lines(f).unwrap(), 4);
    }
}
