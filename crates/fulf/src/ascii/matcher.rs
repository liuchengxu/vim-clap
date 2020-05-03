use {memchr::memchr, std::ops::Index};

type LineMetaData = ();

/// Checks the line, returns `Some()` if it will provide some score.
#[inline]
pub fn matcher(mut line: &[u8], needle: &[u8]) -> Option<LineMetaData> {
    let mut nee_len = needle.len();

    for &letter in needle.iter() {
        let line_len = line.len();
        if line_len < nee_len {
            return None;
        }

        let rcase_idx = if letter.is_ascii_lowercase() {
            memchr(letter.to_ascii_uppercase(), line)
        } else {
            None
        };

        let ocase_idx = memchr(letter, line);

        let next_idx = match (ocase_idx, rcase_idx) {
            (Some(o_idx), Some(r_idx)) => {
                if o_idx < r_idx {
                    o_idx
                } else {
                    r_idx
                }
            }
            (Some(o_idx), None) => o_idx,
            (None, Some(r_idx)) => r_idx,
            (None, None) => return None,
        }
        // ASCII letter length is always 1, so this is always +1.
        +1;

        if next_idx < line_len {
            nee_len = nee_len.saturating_sub(1);
            line = line.index(next_idx..);
        } else {
            return None;
        }
    }

    Some(())
}

/// Checks, if all bytes are ASCII.
///
/// Returns `Some(&str)` if all bytes are ASCII, `None` otherwise.
#[inline]
pub fn ascii_from_bytes(bytes: &[u8]) -> Option<&str> {
    // SAFETY: ASCII is always valid utf8, and bytes are ASCII.
    if bytes.is_ascii() {
        unsafe { Some(std::str::from_utf8_unchecked(bytes)) }
    } else {
        None
    }
}
