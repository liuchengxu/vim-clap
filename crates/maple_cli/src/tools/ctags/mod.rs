mod buffer_tag;
mod context_tag;
mod project_tag;

use std::collections::HashMap;
use std::hash::Hash;
use std::io::{BufRead, BufReader};
use std::ops::Deref;
use std::path::{Path, PathBuf};

use anyhow::Result;
use itertools::Itertools;
use once_cell::sync::Lazy;
use rayon::prelude::*;
use subprocess::{Exec, NullFile};

use crate::paths::AbsPathBuf;
use crate::process::ShellCommand;
use crate::utils::PROJECT_DIRS;

pub use self::buffer_tag::{BufferTag, BufferTagItem};
pub use self::context_tag::{
    buffer_tag_items, buffer_tags_lines, current_context_tag, current_context_tag_async,
};
pub use self::project_tag::{ProjectTag, ProjectTagItem};

pub const EXCLUDE: &str = ".git,*.json,node_modules,target,_build,build,dist";

pub static DEFAULT_EXCLUDE_OPT: Lazy<String> = Lazy::new(|| {
    EXCLUDE
        .split(',')
        .map(|x| format!("--exclude={}", x))
        .join(" ")
});

/// Directory for the `tags` files.
pub static CTAGS_TAGS_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let mut tags_dir = PROJECT_DIRS.data_dir().to_path_buf();
    tags_dir.push("tags");

    std::fs::create_dir_all(&tags_dir).expect("Couldn't create tags directory for vim-clap");

    tags_dir
});

pub static CTAGS_EXISTS: Lazy<bool> = Lazy::new(|| {
    std::process::Command::new("ctags")
        .arg("--version")
        .stderr(std::process::Stdio::inherit())
        .output()
        .ok()
        .and_then(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout
                .split('\n')
                .next()
                .map(|line| line.starts_with("Universal Ctags"))
        })
        .unwrap_or(false)
});

/// If the ctags executable supports `--output-format=json`.
pub static CTAGS_HAS_JSON_FEATURE: Lazy<bool> = Lazy::new(|| {
    fn detect_json_feature() -> std::io::Result<bool> {
        let output = std::process::Command::new("ctags")
            .arg("--list-features")
            .stderr(std::process::Stdio::inherit())
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.split('\n').any(|x| x.starts_with("json")) {
            Ok(true)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "ctags executable has no +json feature",
            ))
        }
    }

    detect_json_feature().unwrap_or(false)
});

/// Used to specify the language when working with `readtags`.
static LANG_MAPS: Lazy<HashMap<String, String>> = Lazy::new(|| {
    fn generate_lang_maps() -> std::io::Result<HashMap<String, String>> {
        let output = std::process::Command::new("ctags")
            .arg("--list-maps")
            .stderr(std::process::Stdio::inherit())
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut lang_maps = HashMap::new();
        for line in stdout.split('\n') {
            let mut items = line.split_whitespace();

            if let Some(lang) = items.next() {
                for item in items {
                    // There are a few edge cases that the item is not like `*.rs`, e.g.,
                    // Asm      *.A51 *.29[kK] *.[68][68][kKsSxX] *.[xX][68][68] *.asm *.ASM *.s *.Shh
                    // it's okay to ignore them and only take care of the most common cases.
                    if let Some(ext) = item.strip_prefix("*.") {
                        lang_maps.insert(ext.to_string(), lang.to_string());
                    }
                }
            }
        }

        Ok(lang_maps)
    }

    generate_lang_maps().unwrap_or_else(|e| {
        tracing::error!(error = ?e, "Failed to initialize LANG_MAPS from `ctags --list-maps`");
        Default::default()
    })
});

/// Returns the ctags language given the file extension.
///
/// So that we can search the tags by specifying the language later.
pub fn get_language(file_extension: &str) -> Option<&str> {
    LANG_MAPS.get(file_extension).map(AsRef::as_ref)
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct TagsGenerator<'a, P> {
    languages: Option<String>,
    kinds_all: &'a str,
    fields: &'a str,
    extras: &'a str,
    exclude_opt: &'a str,
    files: &'a [AbsPathBuf],
    dir: P,
}

impl<'a, P: AsRef<Path> + Hash> TagsGenerator<'a, P> {
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

    pub fn set_languages(&mut self, languages: String) {
        self.languages = Some(languages);
    }

    /// Returns the path of tags file.
    ///
    /// The file path of generated tags is determined by the hash of command itself.
    pub fn tags_path(&self) -> PathBuf {
        let mut tags_path = CTAGS_TAGS_DIR.deref().clone();
        tags_path.push(utility::calculate_hash(self).to_string());
        tags_path
    }

    /// Executes the command to generate the tags file.
    pub fn generate_tags(&self) -> std::io::Result<()> {
        // TODO: detect the languages by dir if not explicitly specified?
        let languages_opt = self
            .languages
            .as_ref()
            .map(|language| format!("--languages={language}"))
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

        let exit_status = Exec::shell(&cmd)
            .stderr(NullFile) // ignore the line: ctags: warning...
            .cwd(self.dir.as_ref())
            .join()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        if !exit_status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Error occured when creating tags file",
            ));
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct ProjectCtagsCommand {
    std_cmd: std::process::Command,
    shell_cmd: ShellCommand,
}

impl ProjectCtagsCommand {
    /// Creates an instance of [`ProjectCtagsCommand`].
    pub fn new(std_cmd: std::process::Command, shell_cmd: ShellCommand) -> Self {
        Self { std_cmd, shell_cmd }
    }

    /// Parallel version of [`formatted_lines`].
    pub fn par_formatted_lines(&mut self) -> std::io::Result<Vec<String>> {
        self.std_cmd.output().map(|output| {
            output
                .stdout
                .par_split(|x| x == &b'\n')
                .filter_map(|tag| {
                    if let Ok(tag) = serde_json::from_slice::<ProjectTag>(tag) {
                        Some(tag.format_proj_tag())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
    }

    pub fn stdout(&mut self) -> Result<Vec<u8>> {
        let stdout = self.std_cmd.output()?.stdout;
        Ok(stdout)
    }

    /// Returns an iterator of raw line of ctags output.
    pub fn lines(&self) -> Result<impl Iterator<Item = String>> {
        let exec_cmd = Exec::cmd(self.std_cmd.get_program())
            .args(self.std_cmd.get_args().collect::<Vec<_>>().as_slice());
        Ok(BufReader::new(exec_cmd.stream_stdout()?).lines().flatten())
    }

    /// Returns an iterator of tag line in a formatted form.
    fn formatted_tags_iter(&self) -> Result<impl Iterator<Item = String>> {
        Ok(self.lines()?.filter_map(|tag| {
            if let Ok(tag) = serde_json::from_str::<ProjectTag>(&tag) {
                Some(tag.format_proj_tag())
            } else {
                None
            }
        }))
    }

    pub fn tag_item_iter(&self) -> Result<impl Iterator<Item = ProjectTagItem>> {
        Ok(self.lines()?.filter_map(|tag| {
            if let Ok(tag) = serde_json::from_str::<ProjectTag>(&tag) {
                Some(tag.into_project_tag_item())
            } else {
                None
            }
        }))
    }

    /// Returns a tuple of (total, cache_path) if the cache exists.
    pub fn ctags_cache(&self) -> Option<(usize, PathBuf)> {
        self.shell_cmd
            .cache_digest()
            .map(|digest| (digest.total, digest.cached_path))
    }

    /// Runs the command and writes the cache to the disk.
    #[allow(unused)]
    fn create_cache(&self) -> Result<(usize, PathBuf)> {
        let mut total = 0usize;
        let mut formatted_tags_iter = self.formatted_tags_iter()?.map(|x| {
            total += 1;
            x
        });
        let lines = formatted_tags_iter.join("\n");

        let cache_path = self
            .shell_cmd
            .clone()
            .write_cache(total, lines.as_bytes())?;

        Ok((total, cache_path))
    }

    /// Parallel version of [`create_cache`].
    pub fn par_create_cache(&mut self) -> Result<(usize, PathBuf)> {
        // TODO: do not store all the output in memory and redirect them to a file directly.
        let lines = self.par_formatted_lines()?;
        let total = lines.len();
        let lines = lines.into_iter().join("\n");

        let cache_path = self
            .shell_cmd
            .clone()
            .write_cache(total, lines.as_bytes())?;

        Ok((total, cache_path))
    }

    pub async fn execute_and_write_cache(mut self) -> Result<Vec<String>> {
        let lines = self.par_formatted_lines()?;

        {
            let lines = lines.clone();

            let total = lines.len();
            let lines = lines.into_iter().join("\n");
            if let Err(e) = self.shell_cmd.clone().write_cache(total, lines.as_bytes()) {
                tracing::error!("Failed to write ctags cache: {e}");
            }
        }

        Ok(lines)
    }
}

// /description/
// /^description$/
pub fn trim_pattern(pattern: &str) -> &str {
    let description = &pattern[1..pattern.len() - 1];

    let description = if let Some(stripped) = description.strip_prefix('^') {
        stripped
    } else {
        description
    };

    let description = if let Some(stripped) = description.strip_suffix('$') {
        stripped
    } else {
        description
    };

    description.trim()
}
