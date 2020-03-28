use {memchr::memchr, smallbitvec::SmallBitVec, std::ops::Index};

type MatcherNeedle<'a> = (usize, &'a [&'a str], &'a [CaseToStr], &'a SmallBitVec);

pub struct NeedleUTF8 {
    owned_str: Box<str>,

    /// This thing points to the data pointed by `owned_str`.
    /// Just make sure you don't use those references after `owned_str` got dropped
    /// (this needs either mutable access to `owned_str` or manual implementation of drop for this struct),
    /// and everything will be okay.
    encoded_chars: Box<[&'static str]>,

    rcased_chars: Box<[CaseToStr]>,

    case_marker: SmallBitVec,
}

impl NeedleUTF8 {
    /// `Some(Self, charcount)` if not empty slice. `None` if empty slice.
    #[inline]
    pub fn new(s: Box<str>) -> Option<(Self, usize)> {
        let len = s.len();
        let mut encoded_idx = Vec::with_capacity(len);
        let mut rcased_chars = Vec::with_capacity(len);
        let mut case_marker = SmallBitVec::new();

        s.char_indices().for_each(|(pos, ch)| {
            encoded_idx.push(pos);

            if ch.is_lowercase() {
                case_marker.push(true);

                rcased_chars.push(CaseToStr::new(ch.to_uppercase()));
            } else {
                case_marker.push(false);
            }
        });

        let charcount = encoded_idx.len();
        if charcount == 0 {
            return None;
        }

        let mut encoded_chars: Vec<&'static str> = Vec::with_capacity(charcount);
        unsafe {
            for idx in 1..charcount {
                let range = *encoded_idx.get_unchecked(idx - 1)..*encoded_idx.get_unchecked(idx);
                let nonstatic: &str = s.get_unchecked(range);
                let stat: &'static str = std::mem::transmute(nonstatic);

                encoded_chars.push(stat);
            }

            let last_idx = *encoded_idx.get_unchecked(charcount - 1);
            let last_nonstatic: &str = s.get_unchecked(last_idx..);
            let last_static: &'static str = std::mem::transmute(last_nonstatic);
            encoded_chars.push(last_static);
        }

        Some((
            Self {
                owned_str: s,
                encoded_chars: encoded_chars.into_boxed_slice(),
                rcased_chars: rcased_chars.into_boxed_slice(),
                case_marker,
            },
            charcount,
        ))
    }

    #[inline]
    pub fn as_matcher_needle<'a>(&'a self) -> MatcherNeedle<'a> {
        (
            self.owned_str.len(),
            self.encoded_chars.as_ref(),
            self.rcased_chars.as_ref(),
            &self.case_marker,
        )
    }
}

impl AsRef<str> for NeedleUTF8 {
    #[inline]
    fn as_ref(&self) -> &str {
        self.owned_str.as_ref()
    }
}

type LineMetaData = ();

/// Checks the line, returns `Some()` if it will provide some score.
///
/// The `needle` here is a length of original `&str`;
/// a bunch of `&str`s, that represent an array of chars encoded as utf8 string;
/// a bunch of reversecased `CaseToStr`s for every lowercase char;
/// a marker for every char, that returns `true` if char has rcase.
#[inline]
pub fn matcher(mut line: &[u8], needle: MatcherNeedle) -> Option<LineMetaData> {
    let (mut nee_len, orig_needle, rcase_needle, rcase_marker) = needle;
    let mut o_iter = orig_needle.iter();
    let mut r_iter = rcase_needle.iter();

    let if_none_return_none = rcase_marker.iter().try_for_each(|r_marker| {
        let line_len = line.len();
        if line_len < nee_len {
            return None;
        }

        let rcase_idx_and_len = if r_marker {
            let cased_utf8_char = r_iter.next().map(|r| r.as_bytes()).unwrap_or(&[]);
            let r_len = cased_utf8_char.len();

            let mut rcase_idx = None;
            match cased_utf8_char {
                &[a] => rcase_idx = memchr(a, line),
                &[a, ..] => {
                    let mut mline = line;

                    while let Some(idx) = memchr(a, mline) {
                        if mline.len() - idx >= r_len {
                            if mline[idx..idx + r_len].eq(cased_utf8_char) {
                                rcase_idx = Some(idx);
                                break;
                            } else {
                                mline = &mline[idx + 1..];
                            }
                        } else {
                            break;
                        }
                    }
                }
                // If there' no reversed case, let it be `None`.
                _ => (),
            }

            rcase_idx.map(|i| (i, r_len))
        } else {
            None
        };

        // This should never produce `None`.
        let utf8_char: &str = o_iter.next().unwrap_or(&"");
        let utf8_char = utf8_char.as_bytes();

        let o_len = utf8_char.len();

        let mut ocase_idx = None;
        match utf8_char {
            &[a] => ocase_idx = memchr(a, line),
            &[a, ..] => {
                let mut mline = line;

                while let Some(idx) = memchr(a, mline) {
                    if mline.len() - idx >= o_len {
                        if mline[idx..idx + o_len].eq(utf8_char) {
                            ocase_idx = Some(idx);
                            break;
                        } else {
                            mline = &mline[idx + 1..];
                        }
                    } else {
                        break;
                    }
                }
            }
            // This last arm should be unreachable.
            _ => (),
        }

        let (next_idx, sub_len) = match (ocase_idx, rcase_idx_and_len) {
            (Some(o_idx), Some((r_idx, r_len))) => {
                let o_next_idx = o_idx + o_len;
                let r_next_idx = r_idx + r_len;
                if o_next_idx < r_next_idx {
                    (o_next_idx, o_len)
                } else {
                    (r_next_idx, r_len)
                }
            }
            (Some(o_idx), None) => (o_idx + o_len, o_len),
            (None, Some((r_idx, r_len))) => (r_idx + r_len, r_len),
            (None, None) => return None,
        };

        if next_idx < line_len {
            nee_len = nee_len.saturating_sub(sub_len);
            line = line.index(next_idx..);
            Some(())
        } else {
            return None;
        }
    });

    match if_none_return_none {
        Some(()) => Some(()),
        None => None,
    }
}

/// A helper struct to translate `ToUpperCase` and `ToLowerCase` into
/// a little stack-allocated buffer of utf-8 encoded bytes.
///
/// # Representation
///
/// First char encoding will always start from the first byte of buffer.
pub struct CaseToStr {
    //XXX SAFETY: this field *must* be of same type with
    //XXX `MAX_LENGTH` constant for safety reasons. Always!
    len: u8,

    buffer: [u8; Self::MAX_LENGTH as _],
}

impl CaseToStr {
    //XXX SAFETY: this constant *must* be of same type with
    //XXX `len` field for safety reasons. Always!
    const MAX_LENGTH: u8 = 12;

    /// Ordinary `new` function.
    //
    // SAFETY: because many methods of this struct use unsafe inside,
    // I describe everything here once.
    //
    // As long as no mutable access is provided, all things down there are true.
    //
    // Index: any indexing is safe if `encode_utf8_iter` provides a proper length
    // as it should.
    //
    // Unicodeness: `encode_utf8_iter` should write proper UTF-8 content,
    // if all chars provided by iterator are real chars, so converting to str is fine.
    #[inline]
    fn new(iter: impl Iterator<Item = char>) -> Self {
        let mut buffer = [0_u8; Self::MAX_LENGTH as _];
        let len = encode_utf8_iter(iter, &mut buffer);

        Self {
            buffer,
            len: len as _,
        }
    }

    #[inline]
    pub fn copy_buffer(&self) -> [u8; Self::MAX_LENGTH as _] {
        self.buffer
    }

    #[inline]
    fn as_str(&self) -> &str {
        //XXX SAFETY: see `new` function.
        unsafe { std::str::from_utf8_unchecked(self.buffer.get_unchecked(..self.len as _)) }
    }
}

impl std::ops::Deref for CaseToStr {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

/// Encodes iterator over `char`s as UTF-8 into the provided byte buffer,
/// and then returns the subslice of the buffer that contains the encoded characters.
///
/// Originally created for converting `ToUpperCase` and `ToLowerCase`.
///
/// # Panics
///
/// Panics if the buffer is not large enough.
/// A buffer of length four is large enough to encode any `char`,
/// thus `4 * iter.len()` is enough to store all possible values.
#[inline]
pub fn encode_utf8_iter<I>(iter: I, mut dst: &mut [u8]) -> usize
where
    I: Iterator<Item = char>,
{
    // UTF-8 ranges and tags for encoding characters
    const TAG_CONT: u8 = 0b1000_0000;
    const TAG_TWO_B: u8 = 0b1100_0000;
    const TAG_THREE_B: u8 = 0b1110_0000;
    const TAG_FOUR_B: u8 = 0b1111_0000;

    let mut total_len = 0;

    for ch in iter {
        let code = ch as u32;
        let len = ch.len_utf8();
        match (len, &mut dst[..]) {
            (1, [a, ..]) => {
                *a = code as u8;
            }
            (2, [a, b, ..]) => {
                *a = (code >> 6 & 0x1F) as u8 | TAG_TWO_B;
                *b = (code & 0x3F) as u8 | TAG_CONT;
            }
            (3, [a, b, c, ..]) => {
                *a = (code >> 12 & 0x0F) as u8 | TAG_THREE_B;
                *b = (code >> 6 & 0x3F) as u8 | TAG_CONT;
                *c = (code & 0x3F) as u8 | TAG_CONT;
            }
            (4, [a, b, c, d, ..]) => {
                *a = (code >> 18 & 0x07) as u8 | TAG_FOUR_B;
                *b = (code >> 12 & 0x3F) as u8 | TAG_CONT;
                *c = (code >> 6 & 0x3F) as u8 | TAG_CONT;
                *d = (code & 0x3F) as u8 | TAG_CONT;
            }
            _ => (), // Next commented line will panic.
        }
        // Will panic here, if `dst` doesn't have enough space.
        dst = &mut dst[len..];
        total_len += len;
    }

    total_len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_to_str() {
        let ch = 'ß';
        let s = CaseToStr::new(ch.to_uppercase());
        assert_eq!(s.as_str(), ch.to_uppercase().collect::<String>().as_str());
    }

    #[test]
    #[should_panic]
    fn test_iter_to_str_panic() {
        let ch = 'ß';
        let mut buf = [0_u8; 1];
        let _a = encode_utf8_iter(ch.to_uppercase(), &mut buf);
        for b in buf.iter() {
            println!("{}", b);
        }
    }
}
