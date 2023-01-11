pub mod vim_help;

use anyhow::Result;
use std::path::Path;
use types::PreviewInfo;
use utility::{read_first_lines, read_preview_lines};

#[inline]
fn as_absolute_path<P: AsRef<Path>>(path: P) -> std::io::Result<String> {
    if path.as_ref().is_absolute() {
        Ok(path.as_ref().to_string_lossy().into())
    } else {
        // Somehow the absolute path on Windows is problematic using `canonicalize`:
        // C:\Users\liuchengxu\AppData\Local\nvim\init.vim
        // \\?\C:\Users\liuchengxu\AppData\Local\nvim\init.vim
        Ok(std::fs::canonicalize(path.as_ref())?
            .into_os_string()
            .to_string_lossy()
            .into())
    }
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
            let replace_start = (0..max_width + 1)
                .rev()
                .find(|idx| line.is_char_boundary(*idx))
                .unwrap_or_default(); // truncate to 0
            line.replace_range(replace_start.., "……");
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
) -> std::io::Result<(Vec<String>, String)> {
    if !path.as_ref().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Can not preview if the object is not a file",
        ));
    }
    let abs_path = as_absolute_path(path.as_ref())?;
    let lines_iter = read_first_lines(path.as_ref(), size)?;
    let lines = std::iter::once(abs_path.clone())
        .chain(truncate_preview_lines(max_width, lines_iter))
        .collect::<Vec<_>>();

    Ok((lines, abs_path))
}

pub fn preview_file_with_truncated_title<P: AsRef<Path>>(
    path: P,
    size: usize,
    max_line_width: usize,
    max_title_width: usize,
) -> std::io::Result<(Vec<String>, String)> {
    let abs_path = as_absolute_path(path.as_ref())?;
    let truncated_abs_path =
        crate::utils::truncate_absolute_path(&abs_path, max_title_width).into_owned();
    let lines_iter = read_first_lines(path.as_ref(), size)?;
    let lines = std::iter::once(truncated_abs_path.clone())
        .chain(truncate_preview_lines(max_line_width, lines_iter))
        .collect::<Vec<_>>();

    Ok((lines, truncated_abs_path))
}

pub fn preview_file_at<P: AsRef<Path>>(
    path: P,
    winheight: usize,
    max_width: usize,
    lnum: usize,
) -> Result<(Vec<String>, usize)> {
    tracing::debug!(path = %path.as_ref().display(), lnum, "Previewing file");

    let PreviewInfo {
        lines,
        highlight_lnum,
        ..
    } = read_preview_lines(path.as_ref(), lnum, winheight)?;

    let lines = std::iter::once(format!("{}:{}", path.as_ref().display(), lnum))
        .chain(truncate_preview_lines(max_width, lines.into_iter()))
        .collect::<Vec<_>>();

    Ok((lines, highlight_lnum))
}
