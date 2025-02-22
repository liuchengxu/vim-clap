use itertools::Itertools;
use matcher::MatchScope;
use serde::{Deserialize, Serialize};
use types::{ClapItem, FuzzyText};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    pub scope: String,
    pub scope_kind: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct BufferTag {
    pub name: String,
    pub pattern: String,
    #[serde(rename = "line")]
    pub line_number: usize,
    pub kind: String,
    #[serde(flatten)]
    pub scope: Option<Scope>,
}

impl BufferTag {
    pub fn trimmed_pattern(&self) -> &str {
        super::trim_pattern(&self.pattern)
    }

    /// Returns the display line for BuiltinHandle, no icon attached.
    pub fn format_buffer_tag(&self, max_name_len: usize) -> String {
        let name_line = format!("{}:{}", self.name, self.line_number);

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

    /// Returns the display line for BuiltinHandle, no icon attached.
    pub fn format_function_tag(&self) -> Option<String> {
        match self.kind.as_str() {
            "field" => None,
            _ => Some(format!(
                "{} {}",
                icon::tags_kind_icon(&self.kind),
                self.name
            )),
        }
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
    pub fn from_json_line(line: &str) -> Option<Self> {
        serde_json::from_str::<Self>(line).ok()
    }

    // The last scope field is optional.
    //
    // Blines	crates/maple_cli/src/app.rs	/^    Blines(command::blines::Blines),$/;"	enumerator	line:39	enum:Cmd
    pub fn from_raw_line(line: &str) -> Option<Self> {
        let mut items = line.split('\t');

        let name = items.next()?.into();
        let _path = items.next()?;

        let mut t = Self {
            name,
            ..Default::default()
        };

        let others = items.join("\t");

        if let Some((tagaddress, kind_line_scope)) = others.rsplit_once(";\"") {
            tagaddress.clone_into(&mut t.pattern);

            let mut iter = kind_line_scope.split_whitespace();

            t.kind = iter.next()?.into();

            t.line_number = iter.next().and_then(|s| {
                s.split_once(':')
                    .and_then(|(_, line)| line.parse::<usize>().ok())
            })?;

            t.scope = iter.next().and_then(|s| {
                s.split_once(':').map(|(scope_kind, scope)| Scope {
                    scope: scope.to_owned(),
                    scope_kind: scope_kind.to_owned(),
                })
            });

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ctags_raw() {
        let line = r#"with_dir	crates/maple_core/src/tools/ctags/mod.rs	/^    pub fn with_dir(dir: P) -> Self {$/;"	method	line:150	implementation:TagsGenerator"#;
        assert_eq!(
            BufferTag::from_raw_line(line).unwrap(),
            BufferTag {
                name: "with_dir".to_string(),
                pattern: "/^    pub fn with_dir(dir: P) -> Self {$/".to_string(),
                line_number: 150,
                kind: "method".to_string(),
                scope: Some(Scope {
                    scope: "TagsGenerator".to_string(),
                    scope_kind: "implementation".to_string(),
                })
            }
        );
    }

    #[test]
    fn test_parse_ctags_json() {
        let json_line = r#"
{"_type": "tag", "name": "with_dir", "path": "crates/maple_core/src/tools/ctags/mod.rs", "pattern": "/^    pub fn with_dir(dir: P) -> Self {$/", "line": 150, "kind": "method", "scope": "TagsGenerator", "scopeKind": "implementation"}
      "#;
        assert_eq!(
            BufferTag::from_json_line(json_line).unwrap(),
            BufferTag {
                name: "with_dir".to_string(),
                pattern: "/^    pub fn with_dir(dir: P) -> Self {$/".to_string(),
                line_number: 150,
                kind: "method".to_string(),
                scope: Some(Scope {
                    scope: "TagsGenerator".to_string(),
                    scope_kind: "implementation".to_string(),
                })
            }
        );
    }
}
