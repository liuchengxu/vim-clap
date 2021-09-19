pub mod cwd;
pub mod filename;
pub mod language;
pub mod recent_files;

use types::MatchingText;

use self::cwd::Cwd;
use self::filename::calc_bonus_filename;
use self::language::Language;
use self::recent_files::RecentFiles;

use crate::Score;

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

    /// Give a bonus if the entry is an absolute file path and matches the cwd.
    Cwd(Cwd),

    /// No additional bonus.
    None,
}

impl Default for Bonus {
    fn default() -> Self {
        Self::None
    }
}

impl<T: AsRef<str>> From<T> for Bonus {
    fn from(s: T) -> Self {
        match s.as_ref().to_lowercase().as_str() {
            "none" => Self::None,
            "filename" => Self::FileName,
            _ => Self::None,
        }
    }
}

impl Bonus {
    /// Constructs a new instance of [`Bonus::Cwd`].
    pub fn cwd(abs_path: String) -> Self {
        Self::Cwd(abs_path.into())
    }

    /// Calculates the bonus score given the match result of base algorithm.
    pub fn bonus_score<'a, T: MatchingText<'a>>(
        &self,
        item: &T,
        score: Score,
        indices: &[usize],
    ) -> Score {
        // Ignore the long line.
        if item.full_text().len() > 1024 {
            return 0;
        }

        let bonus_text = item.bonus_text();

        match self {
            Self::FileName => calc_bonus_filename(bonus_text, score, indices),
            Self::RecentFiles(recent_files) => recent_files.calc_bonus(bonus_text, score),
            Self::Language(language) => language.calc_bonus(bonus_text, score),
            Self::Cwd(cwd) => cwd.calc_bonus(bonus_text, score),
            Self::None => 0,
        }
    }
}
