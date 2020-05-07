//! A custom implementation of `lines()` method.

use {
    memchr::{memchr, memrchr},
    std::{
        iter::{DoubleEndedIterator, FusedIterator, Iterator},
        str,
    },
};

pub enum Line<'a> {
    Ascii(&'a str),
    Utf8(&'a str),
    NotUtf8Line,
}

/// Newline char.
const NL: u8 = b'\n';

#[derive(Clone)]
pub struct ByteLines<'a> {
    text: &'a [u8],
}

//x XXX: poor Windows guys will be left with a '\r' char at the end of a string.
//x Nowadays it's a lone `\n` even on Windows (everywhere except Notepad),
//x so yeah, nobody cares.
impl<'a> ByteLines<'a> {
    #[inline]
    pub fn new(text: &'a [u8]) -> Self {
        Self { text }
    }
}

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
