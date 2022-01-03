use std::collections::HashMap;
use std::hash::Hash;
use std::io::{BufRead, BufReader};
use std::ops::Deref;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use filter::subprocess::Exec;
use itertools::Itertools;
use once_cell::sync::{Lazy, OnceCell};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::paths::AbsPathBuf;
use crate::process::{rstd::StdCommand, BaseCommand};

pub const EXCLUDE: &str = ".git,*.json,node_modules,target,_build,build,dist";

pub static DEFAULT_EXCLUDE_OPT: Lazy<String> = Lazy::new(|| {
    EXCLUDE
        .split(',')
        .map(|x| format!("--exclude={}", x))
        .join(" ")
});

/// Directory for the `tags` files.
pub static TAGS_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let proj_dirs = directories::ProjectDirs::from("org", "vim", "Vim Clap")
        .expect("Couldn't create project directory for vim-clap");

    let mut tags_dir = proj_dirs.data_dir().to_path_buf();
    tags_dir.push("tags");
    std::fs::create_dir_all(&tags_dir).expect("Couldn't create data directory for vim-clap");

    tags_dir
});

/// Used to specify the language when working with `readtags`.
pub static LANG_MAPS: Lazy<HashMap<String, String>> =
    Lazy::new(|| generate_lang_maps().expect("Failed to process the output of `--list-maps`"));

pub fn get_language(extension: &str) -> Option<&str> {
    LANG_MAPS.get(extension).map(AsRef::as_ref)
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct TagsConfig<'a, P> {
    languages: Option<String>,
    kinds_all: &'a str,
    fields: &'a str,
    extras: &'a str,
    exclude_opt: &'a str,
    files: &'a [AbsPathBuf],
    dir: P,
}

impl<'a, P: AsRef<Path> + Hash> TagsConfig<'a, P> {
    pub fn new(
        languages: Option<String>,
        kinds_all: &'a str,
        fields: &'a str,
        extras: &'a str,
        files: &'a [AbsPathBuf],
        dir: P,
        exclude_opt: &'a str,
    ) -> Self {
        Self {
            languages,
            kinds_all,
            fields,
            extras,
            files,
            dir,
            exclude_opt,
        }
    }

    pub fn with_dir(dir: P) -> Self {
        Self {
            languages: None,
            kinds_all: "*",
            fields: "*",
            extras: "*",
            files: Default::default(),
            dir,
            exclude_opt: DEFAULT_EXCLUDE_OPT.deref(),
        }
    }

    pub fn languages(&mut self, languages: String) {
        self.languages = Some(languages);
    }

    /// Returns the path of tags file.
    ///
    /// The file path of generated tags is determined by the hash of command itself.
    pub fn tags_path(&self) -> PathBuf {
        let mut tags_path = TAGS_DIR.deref().clone();
        tags_path.push(utility::calculate_hash(self).to_string());
        tags_path
    }

    fn build_command(&self) -> String {
        // TODO: detect the languages by dir if not explicitly specified?
        let languages_opt = self
            .languages
            .as_ref()
            .map(|v| format!("--languages={}", v))
            .unwrap_or_default();

        let mut cmd = format!(
            "ctags {} --kinds-all='{}' --fields='{}' --extras='{}' {} -f '{}' -R",
            languages_opt,
            self.kinds_all,
            self.fields,
            self.extras,
            self.exclude_opt,
            self.tags_path().display()
        );

        // pass the input files.
        if !self.files.is_empty() {
            cmd.push(' ');
            cmd.push_str(&self.files.iter().map(|f| f.display()).join(" "));
        }

        cmd
    }

    /// Executes the command to generate the tags file.
    pub fn generate_tags(&self) -> Result<()> {
        let command = self.build_command();
        let exit_status = Exec::shell(&command).cwd(self.dir.as_ref()).join()?;

        if !exit_status.success() {
            return Err(anyhow!("Error occured when creating tags file"));
        }

        Ok(())
    }
}

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

fn generate_lang_maps() -> Result<HashMap<String, String>> {
    let output = std::process::Command::new("ctags")
        .arg("--list-maps")
        .stderr(std::process::Stdio::inherit())
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;

    let mut lang_maps = HashMap::new();
    for line in stdout.split('\n') {
        let items = line.split_whitespace().collect::<Vec<_>>();
        if items.len() < 2 {
            continue;
        }
        let lang = String::from(items[0]);
        for ext in &items[1..] {
            if let Some(stripped) = ext.strip_prefix("*.") {
                lang_maps.insert(stripped.to_string(), lang.clone());
            }
        }
    }

    Ok(lang_maps)
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
