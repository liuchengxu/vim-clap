use super::truncate_lines;
use paths::truncate_absolute_path;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use utils::bytelines::ByteLines;
use utils::io::{read_first_lines, FileSizeTier};

/// Preview of a text file.
#[derive(Clone, Debug)]
pub struct TextPreview {
    /// Line number of source file at which the preview starts (exclusive).
    pub start: usize,
    /// Line number of source file at which the preview ends (inclusive).
    pub end: usize,
    /// Total lines in the source file.
    pub total: usize,
    /// 0-based line number of the line that should be highlighted in the preview window.
    pub highlight_lnum: usize,
    /// [start, end] of the source file.
    pub lines: Vec<String>,
}

/// Returns the lines that can fit into the preview window given its window height.
///
/// Center the line at `target_line_number` in the preview window if possible.
/// (`target_line` - `size`, `target_line` - `size`).
pub fn generate_text_preview<P: AsRef<Path>>(
    path: P,
    target_line_number: usize,
    winheight: usize,
) -> std::io::Result<TextPreview> {
    let mid = winheight / 2;
    let (start, end, highlight_lnum) = if target_line_number > mid {
        (target_line_number - mid, target_line_number + mid, mid)
    } else {
        (0, winheight, target_line_number)
    };

    let total = utils::io::line_count(path.as_ref())?;

    let lines = read_lines_in_range(path, start, end)?;

    Ok(TextPreview {
        start,
        end: end.min(total),
        total,
        highlight_lnum,
        lines,
    })
}

fn read_lines_in_range<P: AsRef<Path>>(
    path: P,
    start: usize,
    end: usize,
) -> std::io::Result<Vec<String>> {
    let mut filebuf: Vec<u8> = Vec::new();

    File::open(path)
        .and_then(|mut file| {
            // XXX: is megabyte enough for any text file?
            const MEGABYTE: usize = 32 * 1_048_576;

            let filesize = utils::io::file_size(&file);
            if filesize > MEGABYTE {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "maximum preview file buffer size reached",
                ));
            }

            filebuf.reserve_exact(filesize);
            file.read_to_end(&mut filebuf)
        })
        .map(|_| {
            ByteLines::new(&filebuf)
                .skip(start)
                .take(end - start)
                // trim_end() to get rid of ^M on Windows.
                .map(|l| l.trim_end().to_string())
                .collect()
        })
}

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

fn generate_preview_lines(
    path: impl AsRef<Path>,
    title_line: String,
    max_line_width: usize,
    size: usize,
) -> std::io::Result<(Vec<String>, FileSizeTier)> {
    let file_size = utils::io::determine_file_size_tier(path.as_ref())?;

    let lines = match file_size {
        FileSizeTier::Empty | FileSizeTier::Small => {
            let lines_iter = read_first_lines(path.as_ref(), size)?;
            std::iter::once(title_line)
                .chain(truncate_lines(lines_iter, max_line_width))
                .collect::<Vec<_>>()
        }
        FileSizeTier::Medium => {
            let lines = utils::io::read_lines_from_medium(path.as_ref(), 0, size)?;
            std::iter::once(title_line)
                .chain(truncate_lines(lines.into_iter(), max_line_width))
                .collect::<Vec<_>>()
        }
        FileSizeTier::Large(size) => {
            let size_in_gib = size as f64 / (1024.0 * 1024.0 * 1024.0);
            vec![
                title_line,
                format!("File too large to preview (size: {size_in_gib:.2} GiB)."),
            ]
        }
    };

    Ok((lines, file_size))
}

pub struct TextLines {
    // Preview lines to display.
    pub lines: Vec<String>,
    // Path to display, potentially truncated.
    pub display_path: String,
    // Size tier of the file.
    pub file_size: FileSizeTier,
}

pub fn preview_file<P: AsRef<Path>>(
    path: P,
    size: usize,
    max_line_width: usize,
    max_title_width: Option<usize>,
) -> std::io::Result<TextLines> {
    if !path.as_ref().is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Can not preview if the object is not a file",
        ));
    }

    let abs_path = as_absolute_path(path.as_ref())?;

    let abs_path = if let Some(max_title_width) = max_title_width {
        truncate_absolute_path(&abs_path, max_title_width).into_owned()
    } else {
        abs_path
    };

    let (lines, file_size) = generate_preview_lines(path, abs_path.clone(), max_line_width, size)?;

    Ok(TextLines {
        lines,
        display_path: abs_path,
        file_size,
    })
}

pub fn preview_file_at<P: AsRef<Path>>(
    path: P,
    winheight: usize,
    max_width: usize,
    lnum: usize,
) -> std::io::Result<(Vec<String>, usize)> {
    tracing::debug!(path = %path.as_ref().display(), "Generating preview at line {lnum}");

    let TextPreview {
        lines,
        highlight_lnum,
        ..
    } = generate_text_preview(path.as_ref(), lnum, winheight)?;

    let lines = std::iter::once(format!("{}:{lnum}", path.as_ref().display()))
        .chain(truncate_lines(lines.into_iter(), max_width))
        .collect::<Vec<_>>();

    Ok((lines, highlight_lnum))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_preview_contains_multi_byte() {
        let current_dir = std::env::current_dir().unwrap();
        let root_dir = current_dir.parent().unwrap().parent().unwrap();
        let test_txt = root_dir.join("test").join("testdata").join("test_673.txt");
        let TextPreview { lines, .. } = generate_text_preview(test_txt, 2, 10).unwrap();
        assert_eq!(
            lines,
            [
                "test_ddd",
                "test_ddd    //1����ˤ��ϡ�����1",
                "test_ddd    //2����ˤ��ϡ�����2",
                "test_ddd    //3����ˤ��ϡ�����3",
                "test_ddd    //hello"
            ]
        );
    }
}
