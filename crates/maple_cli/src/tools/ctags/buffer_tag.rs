use itertools::Itertools;
use matcher::{ClapItem, MatchScope};
use serde::{Deserialize, Serialize};
use types::FuzzyText;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct BufferTag {
    pub name: String,
    pub pattern: String,
    pub line: usize,
    pub kind: String,
}

impl BufferTag {
    /// Returns the display line for BuiltinHandle, no icon attached.
    pub fn format_buffer_tag(&self, max_name_len: usize) -> String {
        let name_line = format!("{}:{}", self.name, self.line);

        let kind = format!("[{}]", self.kind);
        let pattern = super::trim_pattern(&self.pattern);
        format!(
            "{name_group:<name_group_width$} {kind:<kind_width$} {pattern}",
            name_group = name_line,
            name_group_width = max_name_len + 6,
            kind = kind,
            kind_width = 10,
        )
    }

    pub fn into_buffer_tag_item(self, max_name_len: usize) -> BufferTagItem {
        let output_text = self.format_buffer_tag(max_name_len);
        BufferTagItem {
            pattern: self.pattern,
            name: self.name,
            output_text,
        }
    }

    #[inline]
    pub fn from_ctags_json(line: &str) -> Option<Self> {
        serde_json::from_str::<Self>(line).ok()
    }

    // The last scope field is optional.
    //
    // Blines	crates/maple_cli/src/app.rs	/^    Blines(command::blines::Blines),$/;"	enumerator	line:39	enum:Cmd
    pub fn from_ctags_raw(line: &str) -> Option<Self> {
        let mut items = line.split('\t');

        let name = items.next()?.into();
        let _path = items.next()?;

        let mut t = Self {
            name,
            ..Default::default()
        };

        let others = items.join("\t");

        if let Some((tagaddress, kind_line_scope)) = others.rsplit_once(";\"") {
            t.pattern = String::from(&tagaddress[2..]);

            let mut iter = kind_line_scope.split_whitespace();

            t.kind = iter.next()?.into();

            t.line = iter.next().and_then(|s| {
                s.split_once(':')
                    .and_then(|(_, line)| line.parse::<usize>().ok())
            })?;

            Some(t)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct BufferTagItem {
    pub pattern: String,
    pub name: String,
    pub output_text: String,
}

impl ClapItem for BufferTagItem {
    fn raw_text(&self) -> &str {
        &self.output_text
    }

    fn match_text(&self) -> &str {
        self.raw_text()
    }

    fn fuzzy_text(&self, _match_scope: MatchScope) -> Option<FuzzyText> {
        Some(FuzzyText::new(&self.name, 0))
    }

    fn bonus_text(&self) -> &str {
        &self.pattern
    }
}
