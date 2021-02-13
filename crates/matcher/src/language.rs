use source_item::SourceItem;

use crate::Score;

#[derive(Debug, Clone)]
pub struct Language(String);

impl From<String> for Language {
    fn from(inner: String) -> Self {
        Self(inner)
    }
}

impl From<&String> for Language {
    fn from(inner: &String) -> Self {
        Self(inner.to_owned())
    }
}

impl From<&str> for Language {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl Language {
    pub fn calc_bonus(&self, item: &SourceItem, base_score: Score) -> Score {
        let trimmed = item.raw.trim_start();
        match self.0.as_str() {
            "vim" => {
                if trimmed.contains("function") {
                    base_score / 3
                } else if trimmed.contains('"') {
                    -(base_score / 5)
                } else {
                    0
                }
            }
            "rs" => {
                if trimmed.contains("fn") {
                    base_score / 3
                } else if trimmed.contains("///") || trimmed.contains("//") {
                    -(base_score / 5)
                } else {
                    0
                }
            }

            _ => 0,
        }
    }
}
