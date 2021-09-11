use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use itertools::Itertools;
use once_cell::sync::OnceCell;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::process::rstd::StdCommand;
use crate::process::BaseCommand;

/// Unit type wrapper of [`BaseCommand`] for ctags.
#[derive(Debug, Clone)]
pub struct CtagsCommand {
    inner: BaseCommand,
}

impl CtagsCommand {
    /// Creates an instance of [`CtagsCommand`].
    pub fn new(inner: BaseCommand) -> Self {
        Self { inner }
    }

    /// Returns an iterator of tag line in a formatted form.
    pub fn formatted_lines(&self) -> Result<Vec<String>> {
        Ok(self
            .run()?
            .filter_map(|tag| {
                if let Ok(tag) = serde_json::from_str::<TagInfo>(&tag) {
                    Some(tag.display_line())
                } else {
                    None
                }
            })
            .collect())
    }

    /// Parallel version of [`formatted_lines`].
    pub fn par_formatted_lines(&self) -> Result<Vec<String>> {
        let stdout = StdCommand::new(&self.inner.command)
            .current_dir(&self.inner.cwd)
            .stdout()?;

        Ok(stdout
            .par_split(|x| x == &b'\n')
            .filter_map(|tag| {
                if let Ok(tag) = serde_json::from_str::<TagInfo>(&String::from_utf8_lossy(tag)) {
                    Some(tag.display_line())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>())
    }

    /// Returns an iterator of raw line of ctags output.
    fn run(&self) -> Result<impl Iterator<Item = String>> {
        Ok(BufReader::new(self.inner.stream_stdout()?)
            .lines()
            .flatten())
    }

    /// Returns an iterator of tag line in a formatted form.
    pub fn formatted_tags_iter(&self) -> Result<impl Iterator<Item = String>> {
        Ok(self.run()?.filter_map(|tag| {
            if let Ok(tag) = serde_json::from_str::<TagInfo>(&tag) {
                Some(tag.display_line())
            } else {
                None
            }
        }))
    }

    /// Returns a tuple of (total, cache_path) if the cache exists.
    pub fn ctags_cache(&self) -> Option<(usize, PathBuf)> {
        self.inner.cache_info()
    }

    /// Runs the command and writes the cache to the disk.
    pub fn create_cache(&self) -> Result<(usize, PathBuf)> {
        let mut total = 0usize;
        let mut formatted_tags_iter = self.formatted_tags_iter()?.map(|x| {
            total += 1;
            x
        });
        let lines = formatted_tags_iter.join("\n");

        let cache_path = self.inner.clone().create_cache(total, lines.as_bytes())?;

        Ok((total, cache_path))
    }

    /// Parallel version of [`create_cache`].
    pub fn par_create_cache(&self) -> Result<(usize, PathBuf)> {
        let lines = self.par_formatted_lines()?;
        let total = lines.len();
        let lines = lines.into_iter().join("\n");

        let cache_path = self.inner.clone().create_cache(total, lines.as_bytes())?;

        Ok((total, cache_path))
    }

    pub async fn create_cache_async(self, lines: Vec<String>) -> Result<()> {
        let total = lines.len();
        let lines = lines.into_iter().join("\n");
        self.inner.create_cache(total, lines.as_bytes())?;
        Ok(())
    }
}

fn detect_json_feature() -> Result<bool> {
    let output = std::process::Command::new("ctags")
        .arg("--list-features")
        .stderr(std::process::Stdio::inherit())
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    if stdout.split('\n').any(|x| x.starts_with("json")) {
        Ok(true)
    } else {
        Err(anyhow!("ctags executable has no +json feature"))
    }
}

/// Returns true if the ctags executable is compiled with +json feature.
pub fn ensure_has_json_support() -> Result<()> {
    static CTAGS_HAS_JSON_FEATURE: OnceCell<bool> = OnceCell::new();
    let json_supported =
        CTAGS_HAS_JSON_FEATURE.get_or_init(|| detect_json_feature().unwrap_or(false));

    if *json_supported {
        Ok(())
    } else {
        Err(anyhow!(
            "The found ctags executable is not compiled with +json feature, please recompile it."
        ))
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct TagInfo {
    name: String,
    path: String,
    pattern: String,
    line: usize,
    kind: String,
}

impl TagInfo {
    /// Builds the line for displaying the tag info.
    pub fn display_line(&self) -> String {
        let pat_len = self.pattern.len();
        let name_lnum = format!("{}:{}", self.name, self.line);
        let kind = format!("[{}@{}]", self.kind, self.path);
        format!(
            "{text:<text_width$} {kind:<kind_width$} {pattern}",
            text = name_lnum,
            text_width = 30,
            kind = kind,
            kind_width = 30,
            pattern = &self.pattern[2..pat_len - 2].trim(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_ctags_line() {
        let data = r#"{"_type": "tag", "name": "Exec", "path": "crates/maple_cli/src/cmd/exec.rs", "pattern": "/^pub struct Exec {$/", "line": 10, "kind": "struct"}"#;
        let tag: TagInfo = serde_json::from_str(&data).unwrap();
        assert_eq!(
            tag,
            TagInfo {
                name: "Exec".into(),
                path: "crates/maple_cli/src/cmd/exec.rs".into(),
                pattern: "/^pub struct Exec {$/".into(),
                line: 10,
                kind: "struct".into()
            }
        );
    }
}
