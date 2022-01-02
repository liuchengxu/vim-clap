use std::hash::Hash;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::Result;
use itertools::Itertools;
use once_cell::sync::Lazy;
use structopt::StructOpt;

use filter::subprocess::Exec;

use super::SharedParams;

use crate::app::Params;
use crate::paths::AbsPathBuf;

#[derive(StructOpt, Debug, Clone)]
struct TagsFileParams {
    /// Same with the `--kinds-all` option of ctags.
    #[structopt(long, default_value = "*")]
    kinds_all: String,

    /// Same with the `--fields` option of ctags.
    #[structopt(long, default_value = "*")]
    fields: String,

    /// Same with the `--extras` option of ctags.
    #[structopt(long, default_value = "*")]
    extras: String,
}

/// Manipulate the tags file.
#[derive(StructOpt, Debug, Clone)]
pub struct TagsFile {
    /// Params for creating tags file.
    #[structopt(flatten)]
    inner: TagsFileParams,

    /// Shared parameters arouns ctags.
    #[structopt(flatten)]
    shared: SharedParams,

    /// Search the tag matching the given query.
    #[structopt(long)]
    query: Option<String>,

    /// Generate the tags file whether the tags file exists or not.
    #[structopt(long)]
    force_generate: bool,

    /// Search the tag case insensitively
    #[structopt(long)]
    #[allow(unused)]
    ignorecase: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct TagsConfig<'a, P> {
    languages: Option<&'a str>,
    kinds_all: &'a str,
    fields: &'a str,
    extras: &'a str,
    exclude_opt: String,
    files: &'a [AbsPathBuf],
    dir: P,
}

/// Represents the manager of tags file.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Tags<'a, P> {
    config: TagsConfig<'a, P>,
    tags_path: PathBuf,
}

pub static TAGS_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let proj_dirs = directories::ProjectDirs::from("org", "vim", "Vim Clap")
        .expect("Couldn't create project directory for vim-clap");

    let mut tags_dir = proj_dirs.data_dir().to_path_buf();
    tags_dir.push("tags");
    std::fs::create_dir_all(&tags_dir).expect("Couldn't create data directory for vim-clap");

    tags_dir
});

impl<'a, P: AsRef<Path> + Hash> TagsConfig<'a, P> {
    pub fn new(
        languages: Option<&'a str>,
        kinds_all: &'a str,
        fields: &'a str,
        extras: &'a str,
        files: &'a [AbsPathBuf],
        dir: P,
        exclude_opt: String,
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
            exclude_opt: super::EXCLUDE
                .split(',')
                .map(|x| format!("--exclude={}", x))
                .join(" "),
        }
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
    fn generate_tags(&self) -> Result<()> {
        let command = self.build_command();
        let exit_status = Exec::shell(&command).cwd(self.dir.as_ref()).join()?;

        if !exit_status.success() {
            return Err(anyhow::anyhow!("Error occured when creating tags file"));
        }

        Ok(())
    }
}

enum FilteringType {
    StartWith,
    Contain,
    Inherit,
}

impl<'a, P: AsRef<Path> + Hash> Tags<'a, P> {
    pub fn new(config: TagsConfig<'a, P>) -> Self {
        let tags_path = config.tags_path();
        Self { config, tags_path }
    }

    /// Returns `true` if the tags file already exists.
    pub fn exists(&self) -> bool {
        self.tags_path.exists()
    }

    pub fn create(&self) -> Result<()> {
        self.config.generate_tags()
    }

    fn build_exec(&self, query: &str, filtering_type: FilteringType) -> Exec {
        // https://docs.ctags.io/en/latest/man/readtags.1.html#examples
        let cmd = Exec::cmd("readtags")
            .arg("--tag-file")
            .arg(&self.tags_path)
            .arg("-E")
            .arg("-ne");

        let cmd = if query.chars().all(char::is_lowercase) {
            cmd.arg("--icase-match")
        } else {
            cmd
        };

        match filtering_type {
            FilteringType::StartWith => cmd.arg("--prefix-match").arg("-").arg(query),
            FilteringType::Contain => cmd
                .arg("-Q")
                .arg(format!("(substr? (downcase $name) \"{}\")", query))
                .arg("-l"),
            FilteringType::Inherit => {
                todo!("Inherit")
            }
        }
    }

    pub fn search(
        &self,
        query: &str,
        force_generate: bool,
    ) -> Result<impl Iterator<Item = TagLine>> {
        use std::io::BufRead;

        if force_generate || !self.exists() {
            self.create()?;
        }

        let stdout = self
            .build_exec(query, FilteringType::StartWith)
            .stream_stdout()?;

        Ok(std::io::BufReader::new(stdout)
            .lines()
            .flatten()
            .filter_map(|line| line.parse::<TagLine>().ok()))
    }
}

#[derive(Default, Debug)]
pub struct TagLine {
    pub name: String,
    pub path: String,
    pub pattern: String,
    pub kind: String,
    pub language: String,
    pub scope: Option<String>,
    pub line: u64,
}

impl FromStr for TagLine {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut items = s.split('\t');

        let mut l = TagLine {
            name: items.next().ok_or(())?.into(),
            path: items.next().ok_or(())?.into(),
            ..Default::default()
        };

        // https://docs.ctags.io/en/latest/man/ctags-client-tools.7.html#parse-readtags-output
        if let Some(p) = items
            .clone()
            .peekable()
            .peek()
            .and_then(|p| p.strip_suffix(";\""))
        {
            let search_pattern_used = (p.starts_with('/') && p.ends_with('/'))
                || (p.len() > 1 && p.starts_with('$') && p.ends_with('$'));
            if search_pattern_used {
                let pat = items.next().ok_or(())?;
                let pat_len = pat.len();
                if p.starts_with("/^") || p.starts_with("?^") {
                    if p.ends_with("$/") || p.ends_with("$?") {
                        l.pattern = String::from(&pat[2..pat_len - 4]);
                    } else {
                        l.pattern = String::from(&pat[2..pat_len - 2]);
                    }
                } else {
                    l.pattern = String::from(&pat[2..pat_len]);
                }
            } else {
                return Err(());
            }
        } else {
            return Err(());
        }

        for item in items {
            if let Some((k, v)) = item.split_once(':') {
                if v.is_empty() {
                    continue;
                }
                match k {
                    "kind" => l.kind = v.into(),
                    "language" => l.language = v.into(),
                    "roles" | "access" => {}
                    "scope" => l.scope = Some(v.into()),
                    "line" => l.line = v.parse().expect("line is an integer"),
                    unknown => {
                        tracing::debug!(line = %s, "Unknown field: {}", unknown);
                    }
                }
            }
        }

        Ok(l)
    }
}

impl TagLine {
    pub fn grep_format(&self, query: &str, ignorecase: bool) -> (String, Option<Vec<usize>>) {
        let mut formatted = format!("[ctags]{}:{}:1:", self.path, self.line);

        let found = if ignorecase {
            self.pattern.to_lowercase().find(&query.to_lowercase())
        } else {
            self.pattern.find(query)
        };

        let indices = if let Some(idx) = found {
            let start = formatted.len() + idx;
            let end = start + query.len();
            Some((start..end).into_iter().collect())
        } else {
            None
        };

        formatted.push_str(&self.pattern);

        (formatted, indices)
    }
}

impl TagsFile {
    pub fn run(&self, _params: Params) -> Result<()> {
        let dir = self.shared.dir()?;

        let config = TagsConfig::new(
            self.shared.languages.as_ref().map(|l| l.as_ref()),
            &self.inner.kinds_all,
            &self.inner.fields,
            &self.inner.extras,
            &self.shared.files,
            &dir,
            self.shared.exclude_opt(),
        );

        let tags = Tags::new(config);

        if let Some(ref query) = self.query {
            let results = tags.search(query, self.force_generate)?;
            for line in results {
                println!("{:?}", line);
            }
        }

        Ok(())
    }
}
