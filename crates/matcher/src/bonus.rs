use source_item::SourceItem;

use crate::{language::Language, Score};

#[derive(Debug, Clone)]
pub enum Bonus {
    /// Give a bonus if the needle matches in the basename of the haystack.
    ///
    /// Ref https://github.com/liuchengxu/vim-clap/issues/561
    FileName,

    /// Give a bonus if the language type is known.
    Language(Language),

    /// Give a bonus if the item is in the list of recently opened files.
    RecentFiles(Vec<String>),

    /// No additional bonus.
    None,
}

impl Default for Bonus {
    fn default() -> Self {
        Self::None
    }
}

impl From<String> for Bonus {
    fn from(b: String) -> Self {
        b.as_str().into()
    }
}

impl From<&str> for Bonus {
    fn from(b: &str) -> Self {
        match b.to_lowercase().as_str() {
            "none" => Self::None,
            "filename" => Self::FileName,
            _ => Self::None,
        }
    }
}

impl From<&String> for Bonus {
    fn from(b: &String) -> Self {
        b.as_str().into()
    }
}

impl Bonus {
    /// Calculates the bonus score given the match result of base algorithm.
    pub fn bonus_for(&self, item: &SourceItem, score: Score, indices: &[usize]) -> Score {
        if item.raw.len() > 1024 {
            return 0;
        }

        match self {
            Bonus::FileName => {
                if let Some((_, idx)) = pattern::file_name_only(&item.raw) {
                    let hits_filename = indices.iter().filter(|x| **x >= idx).count();
                    if item.raw.len() > idx {
                        // bonus = base_score * len(matched elements in filename) / len(filename)
                        score * hits_filename as i64 / (item.raw.len() - idx) as i64
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
            Bonus::RecentFiles(recent_files) => {
                if let Err(bonus) = recent_files.iter().try_for_each(|s| {
                    if s.contains(&item.raw) {
                        let bonus = score / 3;
                        Err(bonus)
                    } else {
                        Ok(())
                    }
                }) {
                    bonus
                } else {
                    0
                }
            }
            Bonus::Language(language) => language.calc_bonus(item, score),
            Bonus::None => 0,
        }
    }
}
