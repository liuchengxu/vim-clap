use chrono::{TimeZone, Utc};
use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::Stdio;

static HUNK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"@@ -(\d+)(,(\d+))? \+(\d+)(,(\d+))? @@").unwrap());

#[derive(Debug)]
pub struct BlameInfo {
    author: String,
    author_time: Option<i64>,
    summary: Option<String>,
}

impl BlameInfo {
    pub fn display(&self, user_name: &str) -> Option<Cow<'_, str>> {
        let author = &self.author;

        if author == "Not Committed Yet" {
            return Some(author.into());
        }

        match (&self.author_time, &self.summary) {
            (Some(author_time), Some(summary)) => {
                let time = Utc.timestamp_opt(*author_time, 0).single()?;
                let time = chrono_humanize::HumanTime::from(time);
                let author = if user_name.eq(author) { "You" } else { author };

                if let Some(fmt) = &maple_config::config().plugin.git.blame_format_string {
                    let mut display_string = fmt.to_string();
                    let mut replace_template_string = |to_replace: &str, replace_with: &str| {
                        if let Some(idx) = display_string.find(to_replace) {
                            display_string.replace_range(idx..idx + to_replace.len(), replace_with);
                        }
                    };

                    replace_template_string("author", author);
                    replace_template_string("time", time.to_string().as_str());
                    replace_template_string("summary", summary);

                    Some(display_string.into())
                } else {
                    Some(format!("({author} {time}) {summary}").into())
                }
            }
            _ => Some(format!("({author})").into()),
        }
    }
}

pub fn parse_blame_info(stdout: Vec<u8>) -> Option<BlameInfo> {
    let stdout = String::from_utf8_lossy(&stdout);

    let mut author = None;
    let mut author_time = None;
    let mut summary = None;

    for line in stdout.split('\n') {
        if let Some((k, v)) = line.split_once(' ') {
            match k {
                "author" => {
                    author.replace(v);
                }
                "author-time" => {
                    author_time.replace(v);
                }
                "summary" => {
                    summary.replace(v);
                }
                _ => {}
            }
        }

        if let (Some(author), Some(author_time), Some(summary)) = (author, author_time, summary) {
            return Some(BlameInfo {
                author: author.to_owned(),
                author_time: Some(author_time.parse::<i64>().expect("invalid author_time")),
                summary: Some(summary.to_owned()),
            });
        }
    }

    None
}

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("untracked")]
    Untracked,
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    FromUtf8(#[from] std::string::FromUtf8Error),
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct Summary {
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HunkSummary {
    Added(usize),
    Removed(usize),
    Modified(usize),
    ModifiedAndAdded { modified: usize, added: usize },
    ModifiedAndRemoved { modified: usize, removed: usize },
}

impl HunkSummary {
    pub fn added(&self) -> usize {
        match self {
            Self::Added(added) | Self::ModifiedAndAdded { added, .. } => *added,
            _ => 0,
        }
    }

    pub fn removed(&self) -> usize {
        match self {
            Self::Removed(removed) | Self::ModifiedAndRemoved { removed, .. } => *removed,
            _ => 0,
        }
    }

    pub fn modified(&self) -> usize {
        match self {
            Self::Modified(modified)
            | Self::ModifiedAndAdded { modified, .. }
            | Self::ModifiedAndRemoved { modified, .. } => *modified,
            _ => 0,
        }
    }
}

/// ChangeType of a hunk.
enum ChangeType {
    Added,
    Removed,
    Modified,
    ModifiedAndAdded,
    ModifiedAndRemoved,
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize)]
pub enum Modification {
    Added(Range<usize>),
    RemovedFirstLine,
    RemovedAboveAndBelow(usize),
    Removed(usize),
    Modified(Range<usize>),
    ModifiedAndAdded {
        modified: Range<usize>,
        added: Range<usize>,
    },
    ModifiedAndRemoved {
        modified: Range<usize>,
        modified_removed: usize,
    },
}

/// Sign types that will be handled on the Vim side.
#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize)]
pub enum SignType {
    #[serde(rename = "A")]
    Added,
    #[serde(rename = "R")]
    Removed,
    #[serde(rename = "M")]
    Modified,
    #[serde(rename = "MR")]
    ModifiedRemoved,
    #[serde(rename = "RA")]
    RemovedAboveAndBelow,
}

/// Returns the intersection of two ranges.
fn intersection(one: &Range<usize>, other: &Range<usize>) -> Option<Range<usize>> {
    let start = one.start.max(other.start);
    let end = one.end.min(other.end);

    // Check if there is a valid intersection
    if start <= end {
        Some(Range { start, end })
    } else {
        None
    }
}

impl Modification {
    pub fn signs_in_range(&self, visual_range: Range<usize>) -> Vec<(usize, SignType)> {
        let mut signs = Vec::new();
        match self {
            Self::Added(added) => {
                if let Some(to_add) = intersection(added, &visual_range) {
                    to_add.for_each(|lnum| signs.push((lnum, SignType::Added)));
                }
            }
            Self::RemovedFirstLine => {
                if visual_range.contains(&1) {
                    signs.push((1, SignType::Removed));
                }
            }
            Self::RemovedAboveAndBelow(lnum) => {
                if visual_range.contains(lnum) {
                    signs.push((*lnum, SignType::RemovedAboveAndBelow));
                }
            }
            Self::Removed(lnum) => {
                if visual_range.contains(lnum) {
                    signs.push((*lnum, SignType::Removed));
                }
            }
            Self::Modified(modified) => {
                if let Some(to_add) = intersection(modified, &visual_range) {
                    to_add.for_each(|lnum| signs.push((lnum, SignType::Modified)));
                }
            }
            Self::ModifiedAndAdded { modified, added } => {
                if let Some(to_add) = intersection(modified, &visual_range) {
                    to_add.for_each(|lnum| signs.push((lnum, SignType::Modified)));
                }

                if let Some(to_add) = intersection(added, &visual_range) {
                    to_add.for_each(|lnum| signs.push((lnum, SignType::Added)));
                }
            }
            Self::ModifiedAndRemoved {
                modified,
                modified_removed,
            } => {
                if let Some(to_add) = intersection(modified, &visual_range) {
                    to_add.for_each(|lnum| signs.push((lnum, SignType::Modified)));
                }

                if visual_range.contains(modified_removed) {
                    signs.push((*modified_removed, SignType::ModifiedRemoved));
                }
            }
        }

        signs
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Hunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
}

impl Hunk {
    /// Returns the summary of this hunk.
    fn summary(&self) -> HunkSummary {
        let from_count = self.old_count;
        let to_count = self.new_count;

        match self.change_type() {
            ChangeType::Added => HunkSummary::Added(to_count),
            ChangeType::Removed => HunkSummary::Removed(from_count),
            ChangeType::Modified => HunkSummary::Modified(to_count),
            ChangeType::ModifiedAndAdded => HunkSummary::ModifiedAndAdded {
                modified: from_count,
                added: to_count - from_count,
            },
            ChangeType::ModifiedAndRemoved => HunkSummary::ModifiedAndRemoved {
                modified: to_count,
                removed: from_count - to_count,
            },
        }
    }

    /// Returns the modification of this hunk.
    fn modification(&self) -> Modification {
        let from_count = self.old_count;
        let to_line = self.new_start;
        let to_count = self.new_count;

        match self.change_type() {
            ChangeType::Added => Modification::Added(to_line..to_line + to_count),
            ChangeType::Removed => {
                if to_line == 0 {
                    Modification::RemovedFirstLine
                } else {
                    Modification::Removed(to_line)
                }
            }
            ChangeType::Modified => Modification::Modified(to_line..to_line + to_count),
            ChangeType::ModifiedAndAdded => Modification::ModifiedAndAdded {
                modified: to_line..to_line + from_count,
                added: to_line + from_count..to_line + to_count,
            },
            ChangeType::ModifiedAndRemoved => Modification::ModifiedAndRemoved {
                modified: to_line..to_line + to_count - 1,
                modified_removed: to_line + to_count - 1,
            },
        }
    }

    // https://github.com/airblade/vim-gitgutter/blob/fe0e8a2630eef548e4122096e4e2241f42208fe3/autoload/gitgutter/diff.vim#L236
    fn change_type(&self) -> ChangeType {
        let from_count = self.old_count;
        let to_count = self.new_count;

        if from_count == 0 && to_count > 0 {
            ChangeType::Added
        } else if from_count > 0 && to_count == 0 {
            ChangeType::Removed
        } else if from_count > 0 && to_count > 0 {
            match from_count.cmp(&to_count) {
                Ordering::Equal => ChangeType::Modified,
                Ordering::Less => ChangeType::ModifiedAndAdded,
                Ordering::Greater => ChangeType::ModifiedAndRemoved,
            }
        } else {
            unreachable!("Unknown hunk type")
        }
    }
}

#[derive(Debug, Clone)]
pub struct GitRepo {
    pub repo: PathBuf,
    pub user_name: String,
}

impl GitRepo {
    pub fn init(git_root: PathBuf) -> Result<Self, GitError> {
        let output = std::process::Command::new("git")
            .current_dir(&git_root)
            .arg("config")
            .arg("user.name")
            .stderr(Stdio::null())
            .output()?;

        let user_name = String::from_utf8(output.stdout)?.trim().to_string();

        Ok(Self {
            repo: git_root,
            user_name,
        })
    }

    pub fn is_tracked(&self, file: &Path) -> std::io::Result<bool> {
        let output = std::process::Command::new("git")
            .current_dir(&self.repo)
            .arg("ls-files")
            .arg("--error-unmatch")
            .arg(file)
            .output()?;
        Ok(output.status.code().map(|c| c != 1).unwrap_or(false))
    }

    pub fn fetch_rev_parse(&self, arg: &str) -> Result<String, GitError> {
        let output = std::process::Command::new("git")
            .current_dir(&self.repo)
            .arg("rev-parse")
            .arg(arg)
            .stderr(Stdio::null())
            .output()?;

        Ok(String::from_utf8(output.stdout)?)
    }

    pub fn fetch_branch(&self) -> Result<String, GitError> {
        let output = std::process::Command::new("git")
            .current_dir(&self.repo)
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD")
            .stderr(Stdio::null())
            .output()?;

        Ok(String::from_utf8(output.stdout)?)
    }

    #[allow(unused)]
    fn fetch_user_name(&self) -> Result<String, GitError> {
        let output = std::process::Command::new("git")
            .current_dir(&self.repo)
            .arg("config")
            .arg("user.name")
            .stderr(Stdio::null())
            .output()?;

        Ok(String::from_utf8(output.stdout)?)
    }

    pub fn fetch_origin_url(&self) -> Result<String, GitError> {
        let output = std::process::Command::new("git")
            .current_dir(&self.repo)
            .arg("config")
            .arg("--get")
            .arg("remote.origin.url")
            .stderr(Stdio::null())
            .output()?;

        Ok(String::from_utf8(output.stdout)?)
    }

    pub fn fetch_blame_output(
        &self,
        relative_path: &Path,
        lnum: usize,
    ) -> std::io::Result<Vec<u8>> {
        let output = std::process::Command::new("git")
            .current_dir(&self.repo)
            .arg("blame")
            .arg("--porcelain")
            .arg("--incremental")
            .arg(format!("-L{lnum},{lnum}"))
            .arg("--")
            .arg(relative_path)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()?;

        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "child process errors out: {}, {}, \
                    command: `git blame --porcelain --incremental -L{lnum},{lnum} -- {}`",
                    String::from_utf8_lossy(&output.stderr),
                    output.status,
                    relative_path.display()
                ),
            ))
        }
    }

    // git blame --contents - -L 100,+1 --line-porcelain crates/maple_core/src/stdio_server/plugin/git.rs
    pub fn fetch_blame_output_with_lines(
        &self,
        relative_path: &Path,
        lnum: usize,
        lines: Vec<String>,
    ) -> std::io::Result<Vec<u8>> {
        let mut p = std::process::Command::new("git")
            .current_dir(&self.repo)
            .arg("blame")
            .arg("--contents")
            .arg("-")
            .arg("-L")
            .arg(format!("{lnum},+1"))
            .arg("--line-porcelain")
            .arg(relative_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = p
            .stdin
            .as_mut()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "stdin unavailable"))?;

        let lines = lines.into_iter().join("\n");
        stdin.write_all(lines.as_bytes())?;

        let output = p.wait_with_output()?;

        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "child process errors out: {}, {}, \
                    command: `git blame --contents - -L {lnum},+1 --line-porcelain {}`",
                    String::from_utf8_lossy(&output.stderr),
                    output.status,
                    relative_path.display()
                ),
            ))
        }
    }

    fn get_hunks(&self, old: &Path, new: Option<&Path>) -> std::io::Result<Vec<Hunk>> {
        let mut cmd = std::process::Command::new("git");
        cmd.current_dir(&self.repo)
            .arg("--no-pager")
            .arg("diff")
            .arg("--no-ext-diff")
            .arg("--no-color")
            .arg("-p")
            .arg("-U0")
            .arg("--")
            .arg(old);

        if let Some(new) = new {
            cmd.arg(new);
        }

        let output = cmd.stdin(Stdio::null()).stderr(Stdio::null()).output()?;

        let output = String::from_utf8_lossy(&output.stdout);

        let hunks = output
            .split('\n')
            .filter_map(|line| {
                if line.starts_with("@@") {
                    parse_hunk(line)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        Ok(hunks)
    }

    pub fn get_diff_summary_and_modifications(
        &self,
        old: &Path,
        new: Option<&Path>,
    ) -> std::io::Result<(Summary, Vec<Modification>)> {
        let hunks = self.get_hunks(old, new)?;

        let mut summary = Summary::default();

        hunks.iter().for_each(|hunk| {
            let hunk_summary = hunk.summary();
            summary.added += hunk_summary.added();
            summary.removed += hunk_summary.removed();
            summary.modified += hunk_summary.modified();
        });

        let modifications = hunks_to_modifications(hunks);

        Ok((summary, modifications))
    }

    pub fn get_diff_summary(&self, old: &Path, new: Option<&Path>) -> std::io::Result<Summary> {
        let hunks = self.get_hunks(old, new)?;

        let mut summary = Summary::default();

        hunks.iter().for_each(|hunk| {
            let hunk_summary = hunk.summary();
            summary.added += hunk_summary.added();
            summary.removed += hunk_summary.removed();
            summary.modified += hunk_summary.modified();
        });

        Ok(summary)
    }

    pub fn get_hunk_modifications(
        &self,
        old: &Path,
        new: Option<&Path>,
    ) -> std::io::Result<Vec<Modification>> {
        Ok(hunks_to_modifications(self.get_hunks(old, new)?))
    }
}

fn hunks_to_modifications(hunks: Vec<Hunk>) -> Vec<Modification> {
    let mut modifications = hunks
        .into_iter()
        .map(|hunk| hunk.modification())
        .collect::<VecDeque<_>>();

    // handle_double_hunks(), https://github.com/airblade/vim-gitgutter/blob/fe0e8a2630eef548e4122096e4e2241f42208fe3/autoload/gitgutter/sign.vim#L209C1-L218C12
    if let (Some(Modification::RemovedFirstLine), Some(Modification::Removed(removed))) =
        (modifications.front(), modifications.get(1))
    {
        if *removed == 1 {
            modifications.pop_front();
            modifications.pop_front();
            modifications.push_front(Modification::RemovedAboveAndBelow(1));
        }
    }

    modifications.into()
}

fn parse_hunk(text: &str) -> Option<Hunk> {
    HUNK.captures(text).and_then(|caps| {
        let old_start = caps.get(1).and_then(|m| m.as_str().parse::<usize>().ok())?;
        let old_count = caps.get(3).and_then(|m| m.as_str().parse::<usize>().ok());
        let new_start = caps.get(4).and_then(|m| m.as_str().parse::<usize>().ok())?;
        let new_count = caps.get(6).and_then(|m| m.as_str().parse::<usize>().ok());
        Some(Hunk {
            old_start,
            old_count: old_count.unwrap_or(1),
            new_start,
            new_count: new_count.unwrap_or(1),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hunk_regex() {
        let line = "@@ -123,0 +143,17 @@ impl ClapPlugin for System {";
        assert_eq!(
            parse_hunk(line).unwrap(),
            Hunk {
                old_start: 123,
                old_count: 0,
                new_start: 143,
                new_count: 17
            }
        );

        let line = "@@ -123 +143 @@ impl ClapPlugin for System {";
        assert_eq!(
            parse_hunk(line).unwrap(),
            Hunk {
                old_start: 123,
                old_count: 1,
                new_start: 143,
                new_count: 1,
            }
        );

        let line = "@@ -123,0 +143 @@ impl ClapPlugin for System {";
        assert_eq!(
            parse_hunk(line).unwrap(),
            Hunk {
                old_start: 123,
                old_count: 0,
                new_start: 143,
                new_count: 1
            }
        );

        let line = "@@ -123 +143,17 @@ impl ClapPlugin for System {";
        assert_eq!(
            parse_hunk(line).unwrap(),
            Hunk {
                old_start: 123,
                old_count: 1,
                new_start: 143,
                new_count: 17
            }
        );
    }
}
