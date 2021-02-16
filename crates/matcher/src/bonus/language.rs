use source_item::SourceItem;

use crate::Score;

pub type FileExtension = String;

#[derive(Debug, Clone)]
pub struct Language(FileExtension);

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

// declaration
// comment

impl Language {
    pub fn calc_bonus(&self, item: &SourceItem, base_score: Score) -> Score {
        let trimmed = item.raw.trim_start();
        match self.0.as_str() {
            "vim" => {
                let calc_bonus = |item: Option<&str>| {
                    item.and_then(|s| {
                        // function[!]
                        if s.starts_with("function") {
                            Some(base_score / 3)
                        } else if s == "let" {
                            Some(base_score / 6)
                        } else if s == "\"" {
                            Some(-(base_score / 5))
                        } else {
                            None
                        }
                    })
                };

                let mut iter = trimmed.split_whitespace();

                // Try the first two items because blines provider prepends the line number to the
                // original line and the language bonus is mostly used in the blines provider.
                let first_item = iter.next();

                match calc_bonus(first_item) {
                    Some(bonus) => bonus,
                    None => {
                        let second_item = iter.next();
                        calc_bonus(second_item).unwrap_or_default()
                    }
                }
            }
            "rs" => {
                let calc_bonus = |item: Option<&str>| {
                    item.and_then(|s| {
                        // pub(crate) fn
                        if s.starts_with("pub") || s == "fn" {
                            Some(base_score / 3)
                        } else if s == "let" {
                            Some(base_score / 6)
                        } else if s.starts_with("//") {
                            Some(-(base_score / 5))
                        } else {
                            None
                        }
                    })
                };

                let mut iter = trimmed.split_whitespace();
                match calc_bonus(iter.next()) {
                    Some(bonus) => bonus,
                    None => calc_bonus(iter.next()).unwrap_or_default(),
                }
            }

            _ => 0,
        }
    }
}
