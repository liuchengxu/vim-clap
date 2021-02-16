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

// declaration
// comment

impl Language {
    pub fn calc_bonus(&self, item: &SourceItem, base_score: Score) -> Score {
        let trimmed = item.raw.trim_start();
        match self.0.as_str() {
            "vim" => {
                let mut iter = trimmed.split_whitespace();
                // Try the first two items because blines provider prepends the line number to the
                // original line and the language bonus is mostly used in the blines provider.
                let first_item = iter.next();

                let try_calc_bonus = |item: Option<&str>| {
                    item.and_then(|s| {
                        // function[!]
                        if s.starts_with("function") {
                            Some(base_score / 3)
                        } else if s == "\"" {
                            Some(-(base_score / 5))
                        } else {
                            None
                        }
                    })
                };

                match try_calc_bonus(first_item) {
                    Some(bonus) => bonus,
                    None => {
                        let second_item = iter.next();
                        try_calc_bonus(second_item).unwrap_or_default()
                    }
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
