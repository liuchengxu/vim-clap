use {
    memchr::memchr,
    std::ops::{Index, IndexMut},
};

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

/// Converts a slice of bytes into valid ASCII-only string in place,
/// replacing any byte that is not ASCII with arbitrary ASCII byte.
///
/// Gives some very strange results, but is very fast.
///
//
// Despite its speed, quite useless thing, really. But was fun to write.
#[inline]
pub fn bytes_to_ascii_strange(s: &mut [u8]) {
    let mut s = s;
    // A little bit hacky.
    //
    // The slice could be divided into three parts:
    // 1. Unaligned (for u32) start.
    // 2. Middle, aligned for u32.
    // 3. End, aligned for u32, but too small.
    let ptr = s.as_ptr();
    let skip = ptr as usize % std::mem::align_of::<u32>();

    const SIZE_U32: usize = std::mem::size_of::<u32>();

    const MASK: u8 = 127;
    // If the last bit is set, the byte is not a valid ASCII.
    // So, to make sure it's valid ASCII, this bit should be unset.
    if s.len() > 18 {
        // Unaligned start.
        for idx in 0..skip {
            let b = s.index_mut(idx);
            *b = *b & MASK;
        }

        unsafe {
            const MASKPACK: u32 = u32::from_ne_bytes([MASK; SIZE_U32]);

            s = s.get_unchecked_mut(skip..);
            let s_len = s.len();
            let (ptr, mid_len) = (s.as_mut_ptr(), s_len / SIZE_U32);

            // Aligned middle.
            let mid = std::slice::from_raw_parts_mut(ptr as *mut u32, mid_len);
            mid.iter_mut()
                .for_each(|packed| *packed = *packed & MASKPACK);

            s = s.index_mut(s_len - s_len % SIZE_U32..);
        }
    }

    // Last part that is too small.
    s.iter_mut().for_each(|b| *b = *b & MASK);
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
