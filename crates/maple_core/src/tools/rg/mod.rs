mod default_types;
mod jsont;
mod stats;

use crate::cache::Digest;
use crate::process::ShellCommand;
use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Command;
use utils::display_width;

pub use self::jsont::{Match, Message, SubMatch};

pub static RG_EXISTS: Lazy<bool> = Lazy::new(|| {
    std::process::Command::new("rg")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .status()
        .map(|exit_status| exit_status.success())
        .unwrap_or(false)
});

/// Map of file extension to ripgrep language.
///
/// https://github.com/BurntSushi/ripgrep/blob/20534fad04/crates/ignore/src/default_types.rs
static RG_LANGUAGE_EXT_TABLE: Lazy<HashMap<&str, &str>> = Lazy::new(|| {
    default_types::DEFAULT_TYPES
        .iter()
        .flat_map(|(lang, values)| {
            values.iter().filter_map(|v| {
                v.split('.').next_back().and_then(|ext| {
                    // Simply ignore the abnormal cases.
                    if ext.contains('[') || ext.contains('*') {
                        None
                    } else {
                        Some((ext, *lang))
                    }
                })
            })
        })
        .collect()
});

/// Finds the ripgrep language given the file extension `ext`.
pub fn get_language(file_extension: &str) -> Option<&&str> {
    RG_LANGUAGE_EXT_TABLE.get(file_extension)
}

/// Word represents the input query around by word boundries.
#[derive(Clone, Debug)]
pub struct Word {
    pub raw: String,
    pub len: usize,
    pub re: regex::Regex,
}

impl Word {
    pub fn new(re_word: String, re: regex::Regex) -> Word {
        Self {
            len: re_word.len(),
            raw: re_word,
            re,
        }
    }

    pub fn find(&self, line: &str) -> Option<usize> {
        self.re.find(line).map(|mat| mat.start())
    }
}

#[inline]
fn range(start: usize, end: usize, offset: usize) -> Range<usize> {
    start + offset..end + offset
}

impl SubMatch {
    pub fn match_indices(&self, offset: usize) -> Range<usize> {
        range(self.start, self.end, offset)
    }

    // FIXME find the word in non-utf8?
    pub fn match_indices_for_dumb_jump(&self, offset: usize, search_word: &Word) -> Range<usize> {
        // The text in SubMatch is not exactly the search word itself in some cases,
        // we need to first find the offset of search word in the SubMatch text manually.
        match search_word.find(&self.m.text()) {
            Some(search_word_offset) => {
                let start = self.start + search_word_offset;
                range(start, start + search_word.len, offset)
            }
            None => Default::default(),
        }
    }
}

impl PartialEq for Match {
    fn eq(&self, other: &Match) -> bool {
        // Ignore the `submatches` field.
        //
        // Given a certain search word, if all the other fields are same, especially the
        // `absolute_offset` equals, these two Match can be considered the same.
        self.path == other.path
            && self.lines == other.lines
            && self.line_number == other.line_number
            && self.absolute_offset == other.absolute_offset
    }
}

impl Eq for Match {}

impl Match {
    pub fn path(&self) -> Cow<str> {
        self.path.text()
    }

    pub fn line_number(&self) -> u64 {
        self.line_number.unwrap_or_default()
    }

    pub fn column(&self) -> usize {
        self.submatches.first().map(|x| x.start).unwrap_or_default()
    }

    /// Returns true if the text line starts with `pat`.
    pub fn line_starts_with(&self, pat: &str) -> bool {
        self.lines.text().trim_start().starts_with(pat)
    }

    pub fn match_indices(&self, offset: usize) -> Vec<usize> {
        self.submatches
            .iter()
            .flat_map(|s| s.match_indices(offset))
            .collect()
    }

    pub fn match_indices_for_dumb_jump(&self, offset: usize, search_word: &Word) -> Vec<usize> {
        self.submatches
            .iter()
            .flat_map(|s| s.match_indices_for_dumb_jump(offset, search_word))
            .collect()
    }
}

impl TryFrom<&[u8]> for Match {
    type Error = Cow<'static, str>;
    fn try_from(byte_line: &[u8]) -> Result<Self, Self::Error> {
        let msg = serde_json::from_slice::<Message>(byte_line)
            .map_err(|e| format!("deserialize error: {e:?}"))?;
        if let Message::Match(mat) = msg {
            Ok(mat)
        } else {
            Err("Not Message::Match type".into())
        }
    }
}

impl TryFrom<&str> for Match {
    type Error = Cow<'static, str>;
    fn try_from(line: &str) -> Result<Self, Self::Error> {
        let msg = serde_json::from_str::<Message>(line)
            .map_err(|e| format!("deserialize error: {e:?}"))?;
        if let Message::Match(mat) = msg {
            Ok(mat)
        } else {
            Err("Not Message::Match type".into())
        }
    }
}

impl Match {
    /// Returns a pair of the formatted `String` and the offset of origin match indices.
    ///
    /// The formatted String is same with the output line using rg's -vimgrep option.
    fn grep_line_format(&self, enable_icon: bool) -> (String, usize) {
        let path = self.path();
        let line_number = self.line_number();
        let column = self.column();
        let pattern = self.pattern();
        let pattern = pattern.trim_end();

        // filepath:line_number:column:text, 3 extra `:` in the formatted String.
        let mut offset =
            path.len() + display_width(line_number as usize) + display_width(column) + 3;

        let formatted_line = if enable_icon {
            let icon = icon::file_icon(&path);
            offset += icon.len_utf8() + 1;
            format!("{icon} {path}:{line_number}:{column}:{pattern}")
        } else {
            format!("{path}:{line_number}:{column}:{pattern}")
        };

        (formatted_line, offset)
    }

    pub fn build_grep_line(&self, enable_icon: bool) -> (String, Vec<usize>) {
        let (formatted, offset) = self.grep_line_format(enable_icon);
        let indices = self.match_indices(offset);
        (formatted, indices)
    }

    #[inline]
    pub fn pattern(&self) -> Cow<str> {
        self.lines.text()
    }

    pub fn pattern_priority(&self) -> code_tools::analyzer::Priority {
        self.path()
            .rsplit_once('.')
            .and_then(|(_, file_ext)| {
                code_tools::analyzer::calculate_pattern_priority(self.pattern(), file_ext)
            })
            .unwrap_or_default()
    }

    /// Returns a pair of the formatted `String` and the offset of matches for dumb_jump provider.
    ///
    /// NOTE: [`pattern::DUMB_JUMP_LINE`] must be updated accordingly once the format is changed.
    fn jump_line_format(&self, kind: &str) -> (String, usize) {
        let path = self.path();
        let line_number = self.line_number();
        let column = self.column();
        let pattern = self.pattern();
        let pattern = pattern.trim_end();

        let formatted_line = format!("[r{kind}]{path}:{line_number}:{column}:{pattern}",);

        let offset = kind.len()
            + path.len()
            + display_width(line_number as usize)
            + display_width(column)
            + 6; // `[r]` + 3 `:`

        (formatted_line, offset)
    }

    pub fn build_jump_line(&self, kind: &str, word: &Word) -> (String, Vec<usize>) {
        let (formatted, offset) = self.jump_line_format(kind);
        let indices = self.match_indices_for_dumb_jump(offset, word);
        (formatted, indices)
    }

    fn jump_line_format_bare(&self) -> (String, usize) {
        let line_number = self.line_number();
        let column = self.column();
        let pattern = self.pattern();
        let pattern = pattern.trim_end();

        let formatted_string = format!("  {line_number}:{column}:{pattern}");

        let offset = display_width(line_number as usize) + display_width(column) + 2 + 2;

        (formatted_string, offset)
    }

    pub fn build_jump_line_bare(&self, word: &Word) -> (String, Vec<usize>) {
        let (formatted, offset) = self.jump_line_format_bare();
        let indices = self.match_indices_for_dumb_jump(offset, word);
        (formatted, indices)
    }
}

const RG_ARGS: &[&str] = &[
    "rg",
    "--column",
    "--line-number",
    "--no-heading",
    "--color=never",
    "--smart-case",
    "",
    ".",
];

// Ref https://github.com/liuchengxu/vim-clap/issues/533
// Now `.` is pushed to the end for all platforms due to https://github.com/liuchengxu/vim-clap/issues/711.
pub const RG_EXEC_CMD: &str =
    "rg --column --line-number --no-heading --color=never --smart-case '' .";

// Used for creating the cache in async context.
#[derive(Debug, Clone, Hash)]
pub struct RgTokioCommand {
    shell_cmd: ShellCommand,
}

impl RgTokioCommand {
    pub fn new(dir: PathBuf) -> Self {
        let shell_cmd = ShellCommand::new(RG_EXEC_CMD.into(), dir);
        Self { shell_cmd }
    }

    pub fn cache_digest(&self) -> Option<Digest> {
        self.shell_cmd.cache_digest()
    }

    pub async fn create_cache(self) -> std::io::Result<Digest> {
        let cache_file = self.shell_cmd.cache_file_path()?;

        let std_cmd = rg_command(&self.shell_cmd.dir);
        let mut tokio_cmd = tokio::process::Command::from(std_cmd);
        crate::process::tokio::write_stdout_to_file(&mut tokio_cmd, &cache_file).await?;

        let digest = crate::cache::store_cache_digest(self.shell_cmd.clone(), cache_file)?;

        Ok(digest)
    }
}

pub fn rg_command<P: AsRef<Path>>(dir: P) -> Command {
    // Can not use StdCommand as it joins the args which does not work somehow.
    let mut cmd = Command::new(RG_ARGS[0]);
    // Do not use --vimgrep here.
    cmd.args(&RG_ARGS[1..]).current_dir(dir);
    cmd
}

pub fn refresh_cache(dir: impl AsRef<Path>) -> std::io::Result<Digest> {
    let shell_cmd = rg_shell_command(dir.as_ref());
    let cache_file_path = shell_cmd.cache_file_path()?;

    let mut cmd = rg_command(dir.as_ref());
    crate::process::write_stdout_to_file(&mut cmd, &cache_file_path)?;

    let digest = crate::cache::store_cache_digest(shell_cmd, cache_file_path)?;

    Ok(digest)
}

#[inline]
pub fn rg_shell_command<P: AsRef<Path>>(dir: P) -> ShellCommand {
    ShellCommand::new(RG_EXEC_CMD.into(), PathBuf::from(dir.as_ref()))
}
