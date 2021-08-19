pub mod recursive;
pub mod tagsfile;

use std::path::PathBuf;

use anyhow::Result;
use itertools::Itertools;
use structopt::StructOpt;

use crate::app::Params;
use crate::paths::AbsPathBuf;

const EXCLUDE: &str = ".git,*.json,node_modules,target,_build";

/// Generate ctags recursively given the directory.
#[derive(StructOpt, Debug, Clone)]
pub(self) struct SharedParams {
    /// The directory for executing the ctags command.
    #[structopt(long, parse(from_os_str))]
    dir: Option<PathBuf>,

    /// Specify the language.
    #[structopt(long)]
    languages: Option<String>,

    /// Exclude files and directories matching 'pattern'.
    ///
    /// Will be translated into ctags' option: --exclude=pattern.
    #[structopt(
        long,
        default_value = EXCLUDE,
        use_delimiter = true
    )]
    exclude: Vec<String>,

    /// Specify the input files.
    // - notify the tags update on demand.
    #[structopt(long)]
    files: Vec<AbsPathBuf>,
}

impl SharedParams {
    pub fn exclude_opt(&self) -> String {
        self.exclude
            .iter()
            .map(|x| format!("--exclude={}", x))
            .join(" ")
    }

    pub fn dir(&self) -> Result<PathBuf> {
        let dir = match self.dir {
            Some(ref d) => d.clone(),
            None => std::env::current_dir()?,
        };

        Ok(dir)
    }
}

/// Ctags command.
#[derive(StructOpt, Debug, Clone)]
pub enum Ctags {
    RecursiveTags(recursive::RecursiveTags),
    TagsFile(tagsfile::TagsFile),
}

impl Ctags {
    pub fn run(&self, params: Params) -> Result<()> {
        match self {
            Self::RecursiveTags(recursive_tags) => recursive_tags.run(params),
            Self::TagsFile(tags_file) => tags_file.run(params),
        }
    }
}
