//! A custom implementation of `lines()` method, display the non-utf8 line as well.

use std::{
    borrow::Cow,
    iter::{DoubleEndedIterator, FusedIterator, Iterator},
    str,
};

use memchr::{memchr, memrchr};

/// Parses raw untrusted bytes into the strings.
#[derive(Clone)]
pub struct ByteLines<'a> {
    text: &'a [u8],
}
impl<'a> ByteLines<'a> {
    #[inline]
    pub fn new(text: &'a [u8]) -> Self {
        Self { text }
    }
}

/// Newline char.
const NL: u8 = b'\n';

impl<'a> Iterator for ByteLines<'a> {
    type Item = Cow<'a, str>;

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // The maximum of items takes every char to be a newline.
        let high = self.text.len();
        (0, Some(high))
    }

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let text = self.text;

        if text.is_empty() {
            return None;
        }

        let line = match memchr(NL, text) {
            Some(newline_idx) => {
                self.text = &text[newline_idx + 1..];
                &text[..newline_idx]
            }

            None => {
                // This line is the last one
                self.text = &[];
                text
            }
        };

        Some(match simdutf8::basic::from_utf8(line) {
            Ok(s) => s.into(),
            Err(_) => String::from_utf8_lossy(line),
        })
    }
}

impl DoubleEndedIterator for ByteLines<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let text = self.text;

        if text.is_empty() {
            return None;
        }

        let line = match memrchr(NL, text) {
            Some(newline_idx) => {
                self.text = &text[newline_idx + 1..];
                &text[..newline_idx]
            }

            None => {
                // This line is the last one
                self.text = &[];
                text
            }
        };

        Some(match simdutf8::basic::from_utf8(line) {
            Ok(s) => s.into(),
            Err(_) => String::from_utf8_lossy(line),
        })
    }
}

impl FusedIterator for ByteLines<'_> {}
