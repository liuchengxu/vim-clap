//! A custom implementation of `lines()` method.

use {
    memchr::{memchr, memrchr},
    std::{
        iter::{DoubleEndedIterator, FusedIterator, Iterator},
        str,
    },
};

/// The result of `ByteLines` parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line<'a> {
    Ascii(&'a str),
    Utf8(&'a str),
    NotUtf8Line,
}

/// Parses raw untrusted bytes into the strings.
///
/// # Examples
///
/// ```
/// use fulf::bytelines::{ByteLines, Line::*};
///
/// let text = concat!("Hello, world!", '\n', "Тнis is пот АSСII-опlу liпе.", '\n');
/// let lines = [text.as_bytes(), &[0_u8, 120, 43, 255, 100]].concat();
/// let mut lines = ByteLines::new(&lines);
/// assert_eq!(lines.next(), Some(Ascii("Hello, world!")));
/// assert_eq!(lines.next(), Some(Utf8("Тнis is пот АSСII-опlу liпе.")));
/// assert_eq!(lines.next(), Some(NotUtf8Line));
/// assert_eq!(lines.next(), None);
/// ```
//
//x XXX: poor Windows guys will be left with a '\r' char at the end of a string.
//x Nowadays it's a lone `\n` even on Windows (everywhere except Notepad),
//x so yeah, nobody cares.
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

        Some(if line.is_ascii() {
            // SAFETY: the whole line is checked and is ASCII,
            // which is always valid utf8.
            unsafe { Line::Ascii(str::from_utf8_unchecked(line)) }
        } else {
            str::from_utf8(line).map_or(Line::NotUtf8Line, Line::Utf8)
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
                self.text = &text[..newline_idx];
                &text[newline_idx + 1..]
            }

            None => {
                // This line is the last one
                self.text = &[];
                text
            }
        };

        Some(if line.is_ascii() {
            // SAFETY: the whole line is checked and is ASCII,
            // which is always valid utf8.
            unsafe { Line::Ascii(str::from_utf8_unchecked(line)) }
        } else {
            str::from_utf8(line).map_or(Line::NotUtf8Line, Line::Utf8)
        })
    }
}

impl FusedIterator for ByteLines<'_> {}
