use crate::bytelines::ByteLines;
use std::fs::{read_dir, remove_dir_all, remove_file, File};
use std::io::{BufRead, BufReader, Error, ErrorKind, Lines, Read, Result};
use std::path::Path;
use types::PreviewInfo;

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

/// Removes all the file and directories under `target_dir`.
pub fn remove_dir_contents<P: AsRef<Path>>(target_dir: P) -> Result<()> {
    let entries = read_dir(target_dir)?;
    for entry in entries.into_iter().flatten() {
        let path = entry.path();

        if path.is_dir() {
            remove_dir_all(path)?;
        } else {
            remove_file(path)?;
        }
    }
    Ok(())
}

/// Returns an Iterator to the Reader of the lines of the file.
///
/// The output is wrapped in a Result to allow matching on errors.
pub fn read_lines<P>(path: P) -> Result<Lines<BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(path)?;
    Ok(BufReader::new(file).lines())
}

/// Returns the first number lines given the file path.
pub fn read_first_lines<P: AsRef<Path>>(
    path: P,
    number: usize,
) -> Result<impl Iterator<Item = String>> {
    let file = File::open(path)?;
    Ok(BufReader::new(file)
        .lines()
        .filter_map(|i| i.ok())
        .take(number))
}

/// Works for utf-8 lines only.
#[allow(unused)]
fn read_preview_lines_utf8<P: AsRef<Path>>(
    path: P,
    target_line: usize,
    size: usize,
) -> Result<(impl Iterator<Item = String>, usize)> {
    let file = File::open(path)?;
    let (start, end, highlight_lnum) = if target_line > size {
        (target_line - size, target_line + size, size)
    } else {
        (0, 2 * size, target_line)
    };
    Ok((
        BufReader::new(file)
            .lines()
            .skip(start)
            .filter_map(|l| l.ok())
            .take(end - start),
        highlight_lnum,
    ))
}

/// Returns the lines that can fit into the preview window given its window height.
///
/// Center the line at `target_line_number` in the preview window if possible.
/// (`target_line` - `size`, `target_line` - `size`).
pub fn read_preview_lines<P: AsRef<Path>>(
    path: P,
    target_line_number: usize,
    winheight: usize,
) -> Result<PreviewInfo> {
    let mid = winheight / 2;
    let (start, end, highlight_lnum) = if target_line_number > mid {
        (target_line_number - mid, target_line_number + mid, mid)
    } else {
        (0, winheight, target_line_number)
    };

    read_preview_lines_impl(path, start, end, highlight_lnum)
}

// Copypasted from stdlib.
/// Indicates how large a buffer to pre-allocate before reading the entire file.
fn initial_buffer_size(file: &File) -> usize {
    // Allocate one extra byte so the buffer doesn't need to grow before the
    // final `read` call at the end of the file.  Don't worry about `usize`
    // overflow because reading will fail regardless in that case.
    file.metadata().map(|m| m.len() as usize + 1).unwrap_or(0)
}

fn read_preview_lines_impl<P: AsRef<Path>>(
    path: P,
    start: usize,
    end: usize,
    highlight_lnum: usize,
) -> Result<PreviewInfo> {
    let mut filebuf: Vec<u8> = Vec::new();

    File::open(path)
        .and_then(|mut file| {
            //x XXX: is megabyte enough for any text file?
            const MEGABYTE: usize = 32 * 1_048_576;

            let filesize = initial_buffer_size(&file);
            if filesize > MEGABYTE {
                return Err(Error::new(
                    ErrorKind::Other,
                    "maximum preview file buffer size reached",
                ));
            }

            filebuf.reserve_exact(filesize);
            file.read_to_end(&mut filebuf)
        })
        .map(|_| {
            let lines = ByteLines::new(&filebuf)
                .skip(start)
                .take(end - start)
                // trim_end() to get rid of ^M on Windows.
                .map(|l| l.trim_end().to_string())
                .collect::<Vec<_>>();

            PreviewInfo {
                start,
                end,
                highlight_lnum,
                lines,
            }
        })
}

/// Returns an iterator of `n` lines of `filename` from the line number `from`.
pub fn read_lines_from<P: AsRef<Path>>(
    path: P,
    from: usize,
    size: usize,
) -> Result<impl Iterator<Item = String>> {
    let file = File::open(path)?;
    Ok(BufReader::new(file)
        .lines()
        .skip(from)
        .filter_map(|i| i.ok())
        .take(size))
}

/// Attempts to write an entire buffer into the file.
///
/// Creates one if the file does not exist.
pub fn create_or_overwrite<P: AsRef<Path>>(path: P, buf: &[u8]) -> Result<()> {
    use std::io::Write;

    // Overwrite it.
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;

    f.write_all(buf)?;
    f.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_byte_reading() {
        let mut current_dir = std::env::current_dir().unwrap();
        current_dir.push("test_673.txt");
        let PreviewInfo { lines, .. } = read_preview_lines(current_dir, 2, 10).unwrap();
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

    #[test]
    fn test_count_lines() {
        let f: &[u8] = b"some text\nwith\nfour\nlines\n";
        assert_eq!(count_lines(f).unwrap(), 4);
    }
}
