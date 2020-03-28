use {memchr::memchr, std::ops::Index};

type LineMetaData = ();

/// Checks the line, returns `Some()` if it will provide some score.
#[inline]
pub fn matcher(mut line: &[u8], needle: &[u8]) -> Option<LineMetaData> {
    let mut nee_len = needle.len();

    for &letter in needle.iter() {
        let line_len = line.len();

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
        };

        if next_idx < line_len {
            nee_len = nee_len.saturating_sub(1);
            line = line.index(next_idx..);
        } else {
            return None;
        }
    }

    Some(())
}

/// Converts a slice of bytes into valid ASCII-only string, returning `String` and
/// replacing any byte that is not ASCII with `?` character.
///
/// Could give some strange result if there's a valid ASCII inside non-valid sequence.
#[inline]
pub fn bytes_into_ascii_string_lossy(s: Vec<u8>) -> String {
    let mut s = s;
    bytes_to_ascii_lossy(s.as_mut());
    unsafe { String::from_utf8_unchecked(s) }
}

/// Converts a slice of bytes into valid ASCII-only string in place,
/// returning `&mut str` and replacing any byte that is not ASCII with `?` character.
///
/// Could give some strange result if there's a valid ASCII inside non-valid sequence.
#[inline]
pub fn bytes_to_ascii_str_lossy(s: &mut [u8]) -> &mut str {
    bytes_to_ascii_lossy(s);
    unsafe { std::str::from_utf8_unchecked_mut(s) }
}

/// Converts a slice of bytes into valid ASCII-only string in place,
/// replacing any byte that is not ASCII with `?` character.
///
/// Could give some strange result if there's a valid ASCII inside non-valid sequence.
#[inline]
pub fn bytes_to_ascii_lossy(s: &mut [u8]) {
    s.iter_mut().for_each(|b| {
        if !b.is_ascii() {
            *b = b'?'
        }
    });
}
