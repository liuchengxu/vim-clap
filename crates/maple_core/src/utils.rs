use anyhow::Result;
use std::io::{BufRead, Lines};
use subprocess::Exec;

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
