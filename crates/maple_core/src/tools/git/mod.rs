use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;
use std::cmp::Ordering;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

static HUNK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"@@ -(\d+)(,(\d+))? \+(\d+)(,(\d+))? @@").unwrap());

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
    pub removed: usize,
    pub modified: usize,
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
    fn summary(&self) -> Summary {
        let Self {
            old_count,
            new_count,
            ..
        } = self;

        let from_count = *old_count;
        let to_count = *new_count;

        let hunk_type = self.hunk_type();

        match hunk_type {
            HunkType::Added => Summary {
                added: to_count,
                ..Default::default()
            },
            HunkType::Removed => Summary {
                removed: from_count,
                ..Default::default()
            },
            HunkType::Modified => Summary {
                modified: to_count,
                ..Default::default()
            },
            HunkType::ModifiedAndAdded => Summary {
                added: to_count - from_count,
                modified: from_count,
                ..Default::default()
            },
            HunkType::ModifiedAndRemoved => Summary {
                modified: to_count,
                removed: from_count - to_count,
                ..Default::default()
            },
        }
    }

    // https://github.com/airblade/vim-gitgutter/blob/fe0e8a2630eef548e4122096e4e2241f42208fe3/autoload/gitgutter/diff.vim#L236
    fn hunk_type(&self) -> HunkType {
        let Self {
            old_count,
            new_count,
            ..
        } = self;

        let from_count = *old_count;
        let to_count = *new_count;

        if from_count == 0 && to_count > 0 {
            HunkType::Added
        } else if from_count > 0 && to_count == 0 {
            HunkType::Removed
        } else if from_count > 0 && to_count > 0 {
            match from_count.cmp(&to_count) {
                Ordering::Equal => HunkType::Modified,
                Ordering::Less => HunkType::ModifiedAndAdded,
                Ordering::Greater => HunkType::ModifiedAndRemoved,
            }
        } else {
            unreachable!("Unknown hunk type")
        }
    }
}

enum HunkType {
    Added,
    Removed,
    Modified,
    ModifiedAndAdded,
    ModifiedAndRemoved,
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

    pub fn get_hunk_summary(&self, old: &Path, new: Option<&Path>) -> std::io::Result<Summary> {
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

        let mut summary = Summary::default();

        hunks.iter().for_each(|hunk| {
            let hunk_summary = hunk.summary();
            summary.added += hunk_summary.added;
            summary.removed += hunk_summary.removed;
            summary.modified += hunk_summary.modified;
        });

        Ok(summary)
    }
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
