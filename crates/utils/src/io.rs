use std::fs::{read_dir, remove_dir_all, remove_file, File};
use std::io::{BufRead, BufReader, Lines, Result};
use std::path::Path;

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

/// Returns the number of total lines of given filepath.
pub fn line_count<P: AsRef<Path>>(path: P) -> std::io::Result<usize> {
    count_lines(std::fs::File::open(path)?)
}

// Copypasted from stdlib.
/// Indicates how large a buffer to pre-allocate before reading the entire file.
pub fn file_size(file: &File) -> usize {
    // Allocate one extra byte so the buffer doesn't need to grow before the
    // final `read` call at the end of the file.  Don't worry about `usize`
    // overflow because reading will fail regardless in that case.
    file.metadata().map(|m| m.len() as usize + 1).unwrap_or(0)
}

/// A utility for checking if the byte size of a file exceeds a specified limit.
#[derive(Debug, Clone)]
pub struct SizeChecker(u64);

impl SizeChecker {
    /// Creates a new [`SizeChecker`] with the size limit.
    pub const fn new(byte_size_limit: u64) -> Self {
        Self(byte_size_limit)
    }

    pub fn size_limit(&self) -> u64 {
        self.0
    }

    /// Checks if the file size exceeds the specified limit.
    pub fn is_too_large<P: AsRef<Path>>(&self, path: P) -> Result<bool> {
        let file = File::open(path.as_ref())?;
        let metadata = file.metadata()?;

        Ok(metadata.len() > self.0)
    }
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

/// Returns the line at given line number (1-based).
pub fn read_line_at<P>(path: P, line_number: usize) -> Result<Option<String>>
where
    P: AsRef<Path>,
{
    Ok(BufReader::new(File::open(path)?)
        .lines()
        .nth(line_number.saturating_sub(1))
        .and_then(Result::ok))
}

/// Returns the first number lines given the file path.
pub fn read_first_lines<P: AsRef<Path>>(
    path: P,
    number: usize,
) -> Result<impl Iterator<Item = String>> {
    read_lines_from(path, 0usize, number)
}

/// Returns a `number` of lines starting from the line number `from` (0-based).
pub fn read_lines_from<P: AsRef<Path>>(
    path: P,
    from: usize,
    number: usize,
) -> Result<impl Iterator<Item = String>> {
    let file = File::open(path)?;
    Ok(BufReader::new(file)
        .lines()
        .skip(from)
        .filter_map(Result::ok)
        .take(number))
}

/// Works for utf-8 lines only.
#[allow(unused)]
fn read_preview_lines_utf8<P: AsRef<Path>>(
    path: P,
    target_line: usize,
    size: usize,
) -> Result<(impl Iterator<Item = String>, usize)> {
    let (start, end, highlight_lnum) = if target_line > size {
        (target_line - size, target_line + size, size)
    } else {
        (0, 2 * size, target_line)
    };
    Ok((read_lines_from(path, start, end - start)?, highlight_lnum))
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
