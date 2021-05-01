pub mod jsont;
pub mod stats;
pub mod util;

use std::convert::TryFrom;
use std::ops::Range;

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
    pub fn path(&self) -> String {
        self.path.text()
    }

    pub fn line_number(&self) -> u64 {
        self.line_number.unwrap_or_default()
    }

    pub fn column(&self) -> usize {
        self.submatches[0].start
    }

    pub fn line(&self) -> String {
        self.lines.text().trim_end().to_owned()
    }

    pub fn match_indices(&self, offset: usize) -> Vec<usize> {
        self.submatches
            .iter()
            .map(|s| s.match_indices(offset))
            .flatten()
            .collect()
    }

    pub fn match_indices_for_dumb_jump(&self, offset: usize, search_word: &Word) -> Vec<usize> {
        self.submatches
            .iter()
            .map(|s| s.match_indices_for_dumb_jump(offset, search_word))
            .flatten()
            .collect()
    }
}

impl TryFrom<&str> for Match {
    type Error = String;
    fn try_from(serialized_str: &str) -> Result<Self, Self::Error> {
        let msg = serde_json::from_str::<Message>(serialized_str)
            .map_err(|e| format!("deserialize error: {:?}", e))?;
        if let Message::Match(mat) = msg {
            Ok(mat)
        } else {
            Err("Not Message::Match type".into())
        }
    }
}

impl Match {
    /// Returns the formatted String like using rg's -vimgrep option.
    pub fn grep_line_format(&self, enable_icon: bool) -> String {
        let maybe_icon = if enable_icon {
            format!("{} ", icon::icon_for(&self.path()))
        } else {
            Default::default()
        };
        format!(
            "{}{}:{}:{}:{}",
            maybe_icon,
            self.path(),
            self.line_number(),
            self.column(),
            self.line(),
        )
    }

    pub fn grep_line_offset(&self, enable_icon: bool) -> usize {
        // filepath:line_number:column:text, 3 extra `:` in the formatted String.
        let fixed_offset = if enable_icon { 3 + 4 } else { 3 };
        self.path().len()
            + self.line_number().to_string().len()
            + self.column().to_string().len()
            + fixed_offset
    }

    pub fn build_grep_line(&self, enable_icon: bool) -> (String, Vec<usize>) {
        let formatted = self.grep_line_format(enable_icon);
        let indices = self.match_indices(self.grep_line_offset(enable_icon));
        (formatted, indices)
    }

    /// NOTE: [`pattern::DUMB_JUMP_LINE`] must be updated accordingly once the format is changed.
    pub fn jump_line_format(&self, kind: &str) -> String {
        format!(
            "[{}]{}:{}:{}:{}",
            kind,
            self.path(),
            self.line_number(),
            self.column(),
            self.line(),
        )
    }

    pub fn jump_line_offset(&self, kind: &str) -> usize {
        self.path().len()
            + self.line_number().to_string().len()
            + self.column().to_string().len()
            + 5 // [] + 3 `:`
            + kind.len()
    }

    pub fn build_jump_line(&self, kind: &str, word: &Word) -> (String, Vec<usize>) {
        let formatted = self.jump_line_format(kind);
        let indices = self.match_indices_for_dumb_jump(self.jump_line_offset(kind), word);
        (formatted, indices)
    }
}
