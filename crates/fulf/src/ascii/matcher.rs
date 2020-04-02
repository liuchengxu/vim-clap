use {
    memchr::memchr,
    std::{mem, ops::Index},
};

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
/// Returns `Ok(&str)` if all bytes are ASCII
/// and `Err(usize)` with index of the first non-ASCII byte, if not.
///
/// Should be faster than default stdlib implementation on big slices.
#[inline]
pub fn ascii_from_bytes(bytes: &[u8]) -> Result<&str, usize> {
    type PackType = u32;

    const PACKSIZE: usize = mem::size_of::<PackType>();
    const MASK: u8 = 128;
    let original_bytes = bytes;

    let mut bytes = bytes;

    let mut index = 0;

    let len = bytes.len();
    if len > PACKSIZE * 4 {
        let align = mem::align_of::<PackType>();
        assert!(align <= PACKSIZE);

        let skip = PACKSIZE - (bytes.as_ptr() as usize % align);

        for idx in 0..skip {
            unsafe {
                let b = *bytes.get_unchecked(idx);
                index += 1;
                if b & MASK != 0 {
                    return Err(index);
                }
            }
        }

        let packmask: PackType = PackType::from_ne_bytes([MASK; PACKSIZE]);

        unsafe {
            // This is aligned and in bounds, look for `assert!` up there
            // right after `len > PACKSIZE * 4` gate.
            bytes = bytes.get_unchecked(skip..);
            let (ptr, bytelen) = (bytes.as_ptr(), bytes.len());
            #[allow(clippy::cast_ptr_alignment)]
            let ptr = ptr as *const PackType;

            let packbytes = std::slice::from_raw_parts(ptr, bytelen / PACKSIZE);

            for &pack in packbytes.iter() {
                if pack & packmask == 0 {
                    index += PACKSIZE;
                } else {
                    let lilbytes = pack.to_ne_bytes();
                    for b in lilbytes.iter() {
                        index += 1;
                        if b & MASK != 0 {
                            return Err(index);
                        }
                    }
                }
            }

            bytes = bytes.get_unchecked(packbytes.len() * PACKSIZE..);
        }
    }

    for &b in bytes.iter() {
        index += 1;
        if b & MASK != 0 {
            return Err(index);
        }
    }

    // SAFETY: just checked all chars and all of them are ASCII.
    Ok(unsafe { std::str::from_utf8_unchecked(original_bytes) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_from_bytes() {
        let any = ascii_from_bytes(b"leaglhewf834u9ruf2qivyfoq8q3bvh fqq38v qyiflqbwhyif ho8yq98dynqofu o8q yhf8h fiqhfi7gq3fiq3gfibi 3fgqogfogqefo78gfi7 geiqf giqgf ieqg ifqeigqip; gpuiqhgiq3pig iqphpiqhg");
        println!("Hello, world!\n {:?}", any);
        let empty = ascii_from_bytes(b"");
        println!("Empty: {:?}", empty);
        let not_ascii = ascii_from_bytes("qwertyhfdbgdfnbйцукен".as_bytes());
        println!("Non-ASCII: {:?}", not_ascii);
    }
}
