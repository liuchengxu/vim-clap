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

/// Returns a `number` of lines starting from the line number `from`.
pub fn read_lines_from<P: AsRef<Path>>(
    path: P,
    from: usize,
    number: usize,
) -> Result<impl Iterator<Item = String>> {
    let file = File::open(path)?;
    Ok(BufReader::new(file)
        .lines()
        .skip(from)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_byte_reading() {
        let mut current_dir = std::env::current_dir().unwrap();
        current_dir.push("test_673.txt");
        let FilePreview { lines, .. } = get_file_preview(current_dir, 2, 10).unwrap();
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
