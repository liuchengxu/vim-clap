pub mod cwd;
pub mod filename;
pub mod language;
pub mod recent_files;

use self::cwd::Cwd;
use self::filename::calc_bonus_file_name;
use self::language::Language;
use self::recent_files::RecentFiles;
use crate::Score;
use std::sync::Arc;
use types::ClapItem;

/// Tweak the matching score calculated by the base match algorithm.
#[derive(Debug, Clone, Default)]
pub enum Bonus {
    /// Give a bonus if the item is an absolute file path and matches the cwd.
    Cwd(Cwd),

    /// Give a bonus if the item contains a language keyword.
    Language(Language),

    /// Give a bonus if the item is in the list of recently opened files.
    RecentFiles(RecentFiles),

    /// Give a bonus if the item is a file path and the matches are in the file name.
    ///
    /// Ref https://github.com/liuchengxu/vim-clap/issues/561
    FileName,

    /// No additional bonus.
    #[default]
    None,
}

impl<T: AsRef<str>> From<T> for Bonus {
    fn from(s: T) -> Self {
        match s.as_ref().to_lowercase().as_str() {
            "filename" => Self::FileName,
            _ => Self::None,
        }
    }
}

impl Bonus {
    /// Calculates the bonus score given the match result of base algorithm.
    pub fn item_bonus_score(
        &self,
        item: &Arc<dyn ClapItem>,
        score: Score,
        indices: &[usize],
    ) -> Score {
        // Ignore the long line.
        if item.raw_text().len() > 1024 {
            return 0;
        }

        self.text_bonus_score(item.bonus_text(), score, indices)
    }

    pub fn text_bonus_score(&self, bonus_text: &str, score: Score, indices: &[usize]) -> Score {
        match self {
            Self::Cwd(cwd) => cwd.calc_bonus(bonus_text, score),
            Self::Language(language) => language.calc_bonus(bonus_text, score),
            Self::RecentFiles(recent_files) => recent_files.calc_bonus(bonus_text, score),
            Self::FileName => calc_bonus_file_name(bonus_text, score, indices),
            Self::None => 0,
        }
    }
}
