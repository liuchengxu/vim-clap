//! A custom implementation of `lines()` method, display the non-utf8 line as well.

use std::{
    fmt::Display,
    iter::{DoubleEndedIterator, FusedIterator, Iterator},
    str,
};

use memchr::{memchr, memrchr};

/// The result of `ByteLines` parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line<'a> {
    Utf8(&'a str),
    NotUtf8(String),
}

impl<'a> Display for Line<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Utf8(line) => write!(f, "{}", line),
            Self::NotUtf8(line) => write!(f, "{}", line),
        }
    }
}

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
    type Item = Line<'a>;

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

        Some(std::str::from_utf8(line).map_or(
            Line::NotUtf8(String::from_utf8_lossy(line).to_string()),
            Line::Utf8,
        ))
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

        Some(std::str::from_utf8(line).map_or(
            Line::NotUtf8(String::from_utf8_lossy(line).to_string()),
            Line::Utf8,
        ))
    }
}

impl FusedIterator for ByteLines<'_> {}
