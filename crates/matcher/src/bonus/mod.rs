pub mod language;
pub mod recent_files;

use types::SourceItem;

use crate::Score;
use language::Language;
use recent_files::RecentFiles;

/// Tweak the matching score calculated by the base match algorithm.
#[derive(Debug, Clone)]
pub enum Bonus {
    /// Give a bonus if the needle matches in the basename of the haystack.
    ///
    /// Ref https://github.com/liuchengxu/vim-clap/issues/561
    FileName,

    /// Give a bonus to for the keywords if the language type is known.
    Language(Language),

    /// Give a bonus if the item is in the list of recently opened files.
    RecentFiles(RecentFiles),

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

fn bonus_for_filename(item: &SourceItem, score: Score, indices: &[usize]) -> Score {
    if let Some((_, idx)) = pattern::file_name_only(&item.raw) {
        if item.raw.len() > idx {
            let hits_filename = indices.iter().filter(|x| **x >= idx).count();
            // bonus = base_score * len(matched elements in filename) / len(filename)
            score * hits_filename as i64 / (item.raw.len() - idx) as i64
        } else {
            0
        }
    } else {
        0
    }
}

impl Bonus {
    /// Calculates the bonus score given the match result of base algorithm.
    pub fn bonus_for(&self, item: &SourceItem, score: Score, indices: &[usize]) -> Score {
        // Ignore the long line.
        if item.raw.len() > 1024 {
            return 0;
        }

        match self {
            Bonus::FileName => bonus_for_filename(item, score, indices),
            Bonus::RecentFiles(recent_files) => recent_files.calc_bonus(item, score),
            Bonus::Language(language) => language.calc_bonus(item, score),
            Bonus::None => 0,
        }
    }
}
