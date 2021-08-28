pub mod cwd;
pub mod filename;
pub mod language;
pub mod recent_files;

use types::SourceItem;

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
    /// Constructs a new instance of [`Bonus::Cwd`].
    pub fn cwd(abs_path: String) -> Self {
        Self::Cwd(abs_path.into())
    }

    /// Calculates the bonus score given the match result of base algorithm.
    pub fn bonus_for(&self, full_line: &str, score: Score, indices: &[usize]) -> Score {
        // Ignore the long line.
        if full_line.len() > 1024 {
            return 0;
        }

        match self {
            Self::FileName => calc_bonus_filename(full_line, score, indices),
            Self::RecentFiles(recent_files) => recent_files.calc_bonus(full_line, score),
            Self::Language(language) => language.calc_bonus(full_line, score),
            Self::Cwd(cwd) => cwd.calc_bonus(full_line, score),
            Self::None => 0,
        }
    }
}
