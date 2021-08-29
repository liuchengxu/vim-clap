//! Add a bonus to the comment line or the line that can have a declaration.
//!
//! Ref: https://github.com/jacktasia/dumb-jump/blob/master/dumb-jump.el

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

/// Calculates the bonus by checking the first two items.
fn calc_bonus_per_item(
    trimmed_line: &str,
    calc_bonus_fn: impl Fn(Option<&str>) -> Option<Score>,
) -> Score {
    let mut iter = trimmed_line.split_whitespace();

    // Try the first two items because blines provider prepends the line number to the
    // original line and the language bonus is mostly used in the blines provider.
    let first_item = iter.next();

    match calc_bonus_fn(first_item) {
        Some(bonus) => bonus,
        None => {
            let second_item = iter.next();
            calc_bonus_fn(second_item).unwrap_or_default()
        }
    }
}

impl Language {
    pub fn calc_bonus(&self, bonus_text: &str, base_score: Score) -> Score {
        let trimmed = bonus_text.trim_start();
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

                calc_bonus_per_item(trimmed, calc_bonus)
            }
            "rs" => {
                const TYPE: [&str; 3] = ["type", "mod", "impl"];
                const FUNCTION: [&str; 2] = ["fn", "macro_rules"];
                const VARIABLE: [&str; 6] = ["let", "const", "static", "enum", "struct", "trait"];

                let calc_bonus = |item: Option<&str>| {
                    item.and_then(|s| {
                        if s.starts_with("pub") {
                            Some(base_score / 6)
                        } else if TYPE.contains(&s) {
                            Some(base_score / 5)
                        } else if FUNCTION.contains(&s) {
                            Some(base_score / 4)
                        } else if VARIABLE.contains(&s) {
                            Some(base_score / 3)
                        } else if s.starts_with("[cfg(feature") {
                            Some(base_score / 7)
                        } else if s.starts_with("//") {
                            Some(-(base_score / 5))
                        } else {
                            None
                        }
                    })
                };

                calc_bonus_per_item(trimmed, calc_bonus)
            }

            _ => 0,
        }
    }
}
