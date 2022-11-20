mod default_types;
mod jsont;
pub mod stats;
pub mod util;

use crate::utils::display_width;
use anyhow::Result;
use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::ops::Range;

pub use self::jsont::{Match, Message, SubMatch};

/// Map of file extension to ripgrep language.
///
/// https://github.com/BurntSushi/ripgrep/blob/20534fad04/crates/ignore/src/default_types.rs
static RG_LANGUAGE_EXT_TABLE: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    default_types::DEFAULT_TYPES
        .iter()
        .flat_map(|(lang, values)| {
            values.iter().filter_map(|v| {
                v.split('.').last().and_then(|ext| {
                    // Simply ignore the abnormal cases.
                    if ext.contains('[') || ext.contains('*') {
                        None
                    } else {
                        Some((ext, *lang))
                    }
                })
            })
        })
        .collect()
});

/// Finds the ripgrep language given the file extension `ext`.
pub fn get_language(file_extension: &str) -> Option<&&str> {
    RG_LANGUAGE_EXT_TABLE.get(file_extension)
}

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
    type Error = Cow<'static, str>;
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
    type Error = Cow<'static, str>;
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

impl Match {
    /// Returns a pair of the formatted `String` and the offset of origin match indices.
    ///
    /// The formatted String is same with the output line using rg's -vimgrep option.
    fn grep_line_format(&self, enable_icon: bool) -> (String, usize) {
        let path = self.path();
        let line_number = self.line_number();
        let column = self.column();
        let pattern = self.pattern();
        let pattern = pattern.trim_end();

        // filepath:line_number:column:text, 3 extra `:` in the formatted String.
        let mut offset =
            path.len() + display_width(line_number as usize) + display_width(column) + 3;

        let formatted_line = if enable_icon {
            let icon = icon::file_icon(&path);
            offset += icon.len_utf8() + 1;
            format!("{icon} {path}:{line_number}:{column}:{pattern}")
        } else {
            format!("{path}:{line_number}:{column}:{pattern}")
        };

        (formatted_line, offset)
    }

    pub fn build_grep_line(&self, enable_icon: bool) -> (String, Vec<usize>) {
        let (formatted, offset) = self.grep_line_format(enable_icon);
        let indices = self.match_indices(offset);
        (formatted, indices)
    }

    #[inline]
    pub fn pattern(&self) -> Cow<str> {
        self.lines.text()
    }

    pub fn pattern_priority(&self) -> dumb_analyzer::Priority {
        self.path()
            .rsplit_once('.')
            .and_then(|(_, file_ext)| {
                dumb_analyzer::calculate_pattern_priority(self.pattern(), file_ext)
            })
            .unwrap_or_default()
    }

    /// Returns a pair of the formatted `String` and the offset of matches for dumb_jump provider.
    ///
    /// NOTE: [`pattern::DUMB_JUMP_LINE`] must be updated accordingly once the format is changed.
    fn jump_line_format(&self, kind: &str) -> (String, usize) {
        let path = self.path();
        let line_number = self.line_number();
        let column = self.column();
        let pattern = self.pattern();
        let pattern = pattern.trim_end();

        let formatted_line = format!("[r{kind}]{path}:{line_number}:{column}:{pattern}",);

        let offset = kind.len()
            + path.len()
            + display_width(line_number as usize)
            + display_width(column)
            + 6; // `[r]` + 3 `:`

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
        let pattern = self.pattern();
        let pattern = pattern.trim_end();

        let formatted_string = format!("  {line_number}:{column}:{pattern}");

        let offset = display_width(line_number as usize) + display_width(column) + 2 + 2;

        (formatted_string, offset)
    }

    pub fn build_jump_line_bare(&self, word: &Word) -> (String, Vec<usize>) {
        let (formatted, offset) = self.jump_line_format_bare();
        let indices = self.match_indices_for_dumb_jump(offset, word);
        (formatted, indices)
    }
}
