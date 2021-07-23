use std::path::Path;

use icon::IconPainter;
use utility::{println_json, println_json_with_length, read_first_lines};

#[derive(Debug, Clone)]
pub enum SendResponse {
    Json,
    JsonWithContentLength,
}

/// Reads the first lines from cache file and send back the cached info.
pub fn send_response_from_cache(
    tempfile: &Path,
    total: usize,
    response_ty: SendResponse,
    icon_painter: Option<IconPainter>,
) {
    let using_cache = true;
    if let Ok(iter) = read_first_lines(&tempfile, 100) {
        let lines: Vec<String> = if let Some(painter) = icon_painter {
            iter.map(|x| painter.paint(&x)).collect()
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
