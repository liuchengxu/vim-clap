use std::hash::Hash;
use std::ops::Deref;
use std::path::{Path, PathBuf};

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

    /// Search the tag case insensitively
    #[structopt(long)]
    #[allow(unused)]
    ignorecase: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct TagsConfig<'a, P> {
    languages: Option<&'a String>,
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
        languages: Option<&'a String>,
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
    fn create(&self) -> Result<()> {
        let command = self.build_command();
        let exit_status = Exec::shell(&command).cwd(self.dir.as_ref()).join()?;

        if !exit_status.success() {
            return Err(anyhow::anyhow!("Error occured when creating tags file"));
        }

        Ok(())
    }
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
        self.config.create()
    }

    pub fn readtags(&self, query: &str) -> Result<impl Iterator<Item = String>> {
        use std::io::BufRead;

        // https://docs.ctags.io/en/latest/man/readtags.1.html#examples
        let stdout = Exec::cmd("readtags")
            .arg("-t")
            .arg(&self.tags_path)
            .arg("-p")
            .arg("-i")
            .arg("-ne")
            .arg("-")
            .arg(query)
            .stream_stdout()?;

        Ok(std::io::BufReader::new(stdout).lines().flatten())
    }
}

impl TagsFile {
    pub fn run(&self, _params: Params) -> Result<()> {
        let dir = self.shared.dir()?;

        let config = TagsConfig::new(
            self.shared.languages.as_ref(),
            &self.inner.kinds_all,
            &self.inner.fields,
            &self.inner.extras,
            &self.shared.files,
            &dir,
            self.shared.exclude_opt(),
        );

        let tags = Tags::new(config);

        if !tags.exists() {
            tags.create()?;
        }

        if let Some(ref query) = self.query {
            for line in tags.readtags(query)?.collect::<Vec<_>>() {
                println!("{}", line);
            }
        }

        Ok(())
    }
}
