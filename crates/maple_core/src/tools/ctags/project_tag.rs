use matcher::MatchScope;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use types::{ClapItem, FuzzyText};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ProjectTag {
    name: String,
    path: String,
    pattern: String,
    line: usize,
    kind: String,
}

impl ProjectTag {
    /// Builds the line for displaying the tag info.
    pub fn format_proj_tag(&self) -> String {
        let name_lnum = format!("{}:{}", self.name, self.line);
        let kind = format!("[{}@{}]", self.kind, self.path);
        let pattern = super::trim_pattern(&self.pattern);
        format!(
            "{text:<text_width$} {kind:<kind_width$} {pattern}",
            text = name_lnum,
            text_width = 30,
            kind = kind,
            kind_width = 30,
        )
    }

    pub fn into_project_tag_item(self) -> ProjectTagItem {
        let output_text = self.format_proj_tag();
        ProjectTagItem {
            name: self.name,
            kind: self.kind,
            output_text,
        }
    }
}

#[derive(Debug)]
pub struct ProjectTagItem {
    pub name: String,
    pub kind: String,
    pub output_text: String,
}

impl ClapItem for ProjectTagItem {
    fn raw_text(&self) -> &str {
        &self.output_text
    }

    fn fuzzy_text(&self, _match_scope: MatchScope) -> Option<FuzzyText<'_>> {
        Some(FuzzyText::new(&self.name, 0))
    }

    fn output_text(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.output_text)
    }

    fn icon(&self, _icon: icon::Icon) -> Option<icon::IconType> {
        Some(icon::tags_kind_icon(&self.kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_project_tag() {
        let data = r#"{"_type": "tag", "name": "Exec", "path": "crates/maple_cli/src/cmd/exec.rs", "pattern": "/^pub struct Exec {$/", "line": 10, "kind": "struct"}"#;
        let tag: ProjectTag = serde_json::from_str(data).unwrap();
        assert_eq!(
            tag,
            ProjectTag {
                name: "Exec".into(),
                path: "crates/maple_cli/src/cmd/exec.rs".into(),
                pattern: "/^pub struct Exec {$/".into(),
                line: 10,
                kind: "struct".into()
            }
        );
    }
}
