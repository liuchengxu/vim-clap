//! This module requires the executable rg with `--json` and `--pcre2` is installed in the system.

use serde::Deserialize;

/// This struct represents the line content of rg's --json.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct JsonLine {
    #[serde(rename = "type")]
    pub ty: String,
    pub data: Match,
}

impl JsonLine {
    /// Returns the formatted String like using rg's -vimgrep option.
    pub fn grep_line_format(&self, enable_icon: bool) -> String {
        let maybe_icon = if enable_icon {
            format!("{} ", icon::icon_for(&self.data.path.text))
        } else {
            Default::default()
        };
        format!(
            "{}{}:{}:{}:{}",
            maybe_icon,
            self.data.path(),
            self.data.line_number(),
            self.data.column(),
            self.data.line(),
        )
    }

    pub fn grep_line_offset(&self, enable_icon: bool) -> usize {
        // filepath:line_number:column:text, 3 extra `:` in the formatted String.
        let fixed_offset = if enable_icon { 3 + 4 } else { 3 };
        self.data.path().len()
            + self.data.line_number().to_string().len()
            + self.data.column().to_string().len()
            + fixed_offset
    }

    pub fn build_grep_line(&self, enable_icon: bool) -> (String, Vec<usize>) {
        let formatted = self.grep_line_format(enable_icon);
        let indices = self.data.match_indices(self.grep_line_offset(enable_icon));
        (formatted, indices)
    }

    /// NOTE: [`pattern::DUMB_JUMP_LINE`] must be updated accordingly once the format is changed.
    pub fn jump_line_format(&self, kind: &str) -> String {
        format!(
            "[{}]{}:{}:{}:{}",
            kind,
            self.data.path(),
            self.data.line_number(),
            self.data.column(),
            self.data.line(),
        )
    }

    pub fn jump_line_offset(&self, kind: &str) -> usize {
        self.data.path().len()
            + self.data.line_number().to_string().len()
            + self.data.column().to_string().len()
            + 5 // [] + 3 `:`
            + kind.len()
    }

    pub fn build_jump_line(&self, kind: &str, word: &Word) -> (String, Vec<usize>) {
        let formatted = self.jump_line_format(kind);
        let indices = self
            .data
            .match_indices_for_dumb_jump(self.jump_line_offset(kind), word);
        (formatted, indices)
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Text {
    pub text: String,
}

#[derive(Deserialize, Clone, Debug, Eq)]
pub struct Match {
    pub path: Text,
    pub lines: Text,
    pub line_number: Option<u64>,
    pub absolute_offset: u64,
    pub submatches: Vec<SubMatch>,
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

impl Match {
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

    pub fn path(&self) -> &str {
        &self.path.text
    }

    pub fn line_number(&self) -> u64 {
        self.line_number.unwrap_or_default()
    }

    pub fn column(&self) -> usize {
        self.submatches[0].start
    }

    pub fn line(&self) -> &str {
        self.lines.text.trim_end()
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SubMatch {
    #[serde(rename = "match")]
    pub m: Text,
    pub start: usize,
    pub end: usize,
}

#[inline]
fn range(start: usize, end: usize, offset: usize) -> Vec<usize> {
    (start + offset..end + offset).into_iter().collect()
}

impl SubMatch {
    pub fn match_indices(&self, offset: usize) -> Vec<usize> {
        range(self.start, self.end, offset)
    }

    pub fn match_indices_for_dumb_jump(&self, offset: usize, search_word: &Word) -> Vec<usize> {
        // The text in SubMatch is not exactly the search word itself in some cases,
        // we need to first find the offset of search word in the SubMatch text manually.
        match search_word.find(&self.m.text) {
            Some(search_word_offset) => {
                let start = self.start + search_word_offset;
                range(start, start + search_word.len, offset)
            }
            None => Default::default(),
        }
    }
}

/// Word represents the input query around by word boundries.
#[derive(Clone, Debug)]
pub struct Word {
    pub raw: String,
    pub len: usize,
    pub re: regex::Regex,
}

impl Word {
    pub fn new(word: String) -> Word {
        let re = regex::Regex::new(&format!("\\b{}\\b", word)).unwrap();
        Self {
            len: word.len(),
            raw: word,
            re,
        }
    }

    pub fn find(&self, line: &str) -> Option<usize> {
        self.re.find(line).map(|mat| mat.start())
    }
}

#[test]
fn test_search_word_is_a_word() {
    let line = "fn unchecked_from(h: crate::hash::H256)";
    let re = regex::Regex::new(r"\bh\b").unwrap();
    let mat = re.find(line).unwrap();
    assert_eq!(mat.start(), 18);
}
