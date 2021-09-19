pub mod jsont;
pub mod stats;
pub mod util;

use std::ops::Range;
use std::{borrow::Cow, convert::TryFrom};

use anyhow::Result;

pub use self::jsont::{Match, Message, SubMatch};

/// Word represents the input query around by word boundries.
#[derive(Clone, Debug)]
pub struct Word {
    pub raw: String,
    pub len: usize,
    pub re: regex::Regex,
}

impl Word {
    pub fn new(word: String) -> Result<Word> {
        let re = regex::Regex::new(&format!("\\b{}\\b", word))?;
        Ok(Self {
            len: word.len(),
            raw: word,
            re,
        })
    }

    pub fn find(&self, line: &str) -> Option<usize> {
        self.re.find(line).map(|mat| mat.start())
    }
}

#[inline]
fn range(start: usize, end: usize, offset: usize) -> Range<usize> {
    start + offset..end + offset
}

impl SubMatch {
    pub fn match_indices(&self, offset: usize) -> Range<usize> {
        range(self.start, self.end, offset)
    }

    // FIXME find the word in non-utf8?
    pub fn match_indices_for_dumb_jump(&self, offset: usize, search_word: &Word) -> Range<usize> {
        // The text in SubMatch is not exactly the search word itself in some cases,
        // we need to first find the offset of search word in the SubMatch text manually.
        match search_word.find(&self.m.text()) {
            Some(search_word_offset) => {
                let start = self.start + search_word_offset;
                range(start, start + search_word.len, offset)
            }
            None => Default::default(),
        }
    }
}

impl PartialEq for Match {
    fn eq(&self, other: &Match) -> bool {
        // Ignore the `submatches` field.
        //
        // Given a certain search word, if all the other fields are same, especially the
        // `absolute_offset` equals, these two Match can be considered the same.
        self.path == other.path
            && self.lines == other.lines
            && self.line_number == other.line_number
            && self.absolute_offset == other.absolute_offset
    }
}

impl Eq for Match {}

impl Match {
    pub fn path(&self) -> Cow<str> {
        self.path.text()
    }

    pub fn line_number(&self) -> u64 {
        self.line_number.unwrap_or_default()
    }

    pub fn column(&self) -> usize {
        self.submatches.get(0).map(|x| x.start).unwrap_or_default()
    }

    /// Returns true if the text line starts with `pat`.
    pub fn line_starts_with(&self, pat: &str) -> bool {
        self.lines.text().trim_start().starts_with(pat)
    }

    pub fn match_indices(&self, offset: usize) -> Vec<usize> {
        self.submatches
            .iter()
            .flat_map(|s| s.match_indices(offset))
            .collect()
    }

    pub fn match_indices_for_dumb_jump(&self, offset: usize, search_word: &Word) -> Vec<usize> {
        self.submatches
            .iter()
            .flat_map(|s| s.match_indices_for_dumb_jump(offset, search_word))
            .collect()
    }
}

impl TryFrom<&[u8]> for Match {
    type Error = String;
    fn try_from(byte_line: &[u8]) -> Result<Self, Self::Error> {
        let msg = serde_json::from_slice::<Message>(byte_line)
            .map_err(|e| format!("deserialize error: {:?}", e))?;
        if let Message::Match(mat) = msg {
            Ok(mat)
        } else {
            Err("Not Message::Match type".into())
        }
    }
}

impl TryFrom<&str> for Match {
    type Error = String;
    fn try_from(line: &str) -> Result<Self, Self::Error> {
        let msg = serde_json::from_str::<Message>(line)
            .map_err(|e| format!("deserialize error: {:?}", e))?;
        if let Message::Match(mat) = msg {
            Ok(mat)
        } else {
            Err("Not Message::Match type".into())
        }
    }
}

/// Returns the width of displaying `n` on the screen.
///
/// Same with `n.to_string().len()` but without allocation.
fn display_width(mut n: usize) -> usize {
    if n == 0 {
        return 1;
    }

    let mut len = 0;
    while n > 0 {
        len += 1;
        n /= 10;
    }

    len
}

impl Match {
    /// Returns a pair of the formatted `String` and the offset of origin match indices.
    ///
    /// The formatted String is same with the output line using rg's -vimgrep option.
    fn grep_line_format(&self, enable_icon: bool) -> (String, usize) {
        let path = self.path();
        let line_number = self.line_number();
        let column = self.column();

        let maybe_icon = if enable_icon {
            format!("{} ", icon::file_icon(&path))
        } else {
            Default::default()
        };

        let formatted_line = format!(
            "{}{}:{}:{}:{}",
            maybe_icon,
            path,
            line_number,
            column,
            self.lines.text().trim_end()
        );

        // filepath:line_number:column:text, 3 extra `:` in the formatted String.
        let fixed_offset = if enable_icon { 3 + 4 } else { 3 };

        let offset =
            path.len() + display_width(line_number as usize) + display_width(column) + fixed_offset;

        (formatted_line, offset)
    }

    pub fn build_grep_line(&self, enable_icon: bool) -> (String, Vec<usize>) {
        let (formatted, offset) = self.grep_line_format(enable_icon);
        let indices = self.match_indices(offset);
        (formatted, indices)
    }

    /// Returns a pair of the formatted `String` and the offset of matches for dumb_jump provider.
    ///
    /// NOTE: [`pattern::DUMB_JUMP_LINE`] must be updated accordingly once the format is changed.
    fn jump_line_format(&self, kind: &str) -> (String, usize) {
        let path = self.path();
        let line_number = self.line_number();
        let column = self.column();

        let formatted_line = format!(
            "[{}]{}:{}:{}:{}",
            kind,
            path,
            line_number,
            column,
            self.lines.text().trim_end()
        );

        let offset = path.len()
            + display_width(line_number as usize)
            + display_width(column)
            + 5 // [] + 3 `:`
            + kind.len();

        (formatted_line, offset)
    }

    pub fn build_jump_line(&self, kind: &str, word: &Word) -> (String, Vec<usize>) {
        let (formatted, offset) = self.jump_line_format(kind);
        let indices = self.match_indices_for_dumb_jump(offset, word);
        (formatted, indices)
    }

    fn jump_line_format_bare(&self) -> (String, usize) {
        let line_number = self.line_number();
        let column = self.column();

        let formatted_string = format!(
            "  {}:{}:{}",
            line_number,
            column,
            self.lines.text().trim_end()
        );

        let offset = display_width(line_number as usize) + display_width(column) + 2 + 2;

        (formatted_string, offset)
    }

    pub fn build_jump_line_bare(&self, word: &Word) -> (String, Vec<usize>) {
        let (formatted, offset) = self.jump_line_format_bare();
        let indices = self.match_indices_for_dumb_jump(offset, word);
        (formatted, indices)
    }
}
