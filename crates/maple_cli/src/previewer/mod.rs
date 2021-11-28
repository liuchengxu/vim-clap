pub mod vim_help;

use std::path::Path;

use anyhow::{anyhow, Result};

use utility::{read_first_lines, read_preview_lines};

#[inline]
pub fn as_absolute_path<P: AsRef<Path>>(path: P) -> Result<String> {
    std::fs::canonicalize(path.as_ref())?
        .into_os_string()
        .into_string()
        .map_err(|e| anyhow!("{:?}, path:{}", e, path.as_ref().display()))
}

/// Truncates the lines that are awfully long as vim can not handle them properly.
///
/// Ref https://github.com/liuchengxu/vim-clap/issues/543
pub fn truncate_preview_lines(
    max_width: usize,
    lines: impl Iterator<Item = String>,
) -> impl Iterator<Item = String> {
    lines.map(move |line| {
        if line.len() > max_width {
            let mut line = line;
            // https://github.com/liuchengxu/vim-clap/pull/544#discussion_r506281014
            line.truncate(
                (0..max_width + 1)
                    .rev()
                    .find(|idx| line.is_char_boundary(*idx))
                    .unwrap_or_default(), // truncate to 0
            );
            line.push_str("……");
            line
        } else {
            line
        }
    })
}

pub fn preview_file<P: AsRef<Path>>(
    path: P,
    size: usize,
    max_width: usize,
) -> Result<(Vec<String>, String)> {
    let abs_path = as_absolute_path(path.as_ref())?;
    let lines_iter = read_first_lines(path.as_ref(), size)?;
    let lines = std::iter::once(abs_path.clone())
        .chain(truncate_preview_lines(max_width, lines_iter))
        .collect::<Vec<_>>();

    Ok((lines, abs_path))
}

pub fn preview_file_at<P: AsRef<Path> + std::fmt::Debug>(
    path: P,
    half_size: usize,
    max_width: usize,
    lnum: usize,
) -> Result<(Vec<String>, usize)> {
    tracing::debug!(?path, lnum, "Previewing file");

    let (lines_iter, hi_lnum) = read_preview_lines(path.as_ref(), lnum, half_size)?;
    let lines = std::iter::once(format!("{}:{}", path.as_ref().display(), lnum))
        .chain(truncate_preview_lines(max_width, lines_iter.into_iter()))
        .collect::<Vec<_>>();

    Ok((lines, hi_lnum))
}
