pub mod buffer_tags;
pub mod recursive_tags;
pub mod tags_file;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use itertools::Itertools;

use crate::app::Params;
use crate::paths::AbsPathBuf;
use crate::tools::ctags::EXCLUDE;

/// Generate ctags recursively given the directory.
#[derive(Parser, Debug, Clone)]
pub struct SharedParams {
    /// The directory for executing the ctags command.
    #[clap(long, parse(from_os_str))]
    dir: Option<PathBuf>,

    /// Specify the language.
    #[clap(long)]
    languages: Option<String>,

    /// Exclude files and directories matching 'pattern'.
    ///
    /// Will be translated into ctags' option: --exclude=pattern.
    #[clap(
        long,
        default_value = EXCLUDE,
        use_value_delimiter = true
    )]
    exclude: Vec<String>,

    /// Specify the input files.
    // - notify the tags update on demand.
    #[clap(long)]
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
#[derive(Subcommand, Debug, Clone)]
pub enum Ctags {
    BufferTags(buffer_tags::BufferTags),
    RecursiveTags(recursive_tags::RecursiveTags),
    TagsFile(tags_file::TagsFile),
}

impl Ctags {
    pub fn run(&self, params: Params) -> Result<()> {
        match self {
            Self::BufferTags(buffer_tags) => buffer_tags.run(params),
            Self::RecursiveTags(recursive_tags) => recursive_tags.run(params),
            Self::TagsFile(tags_file) => tags_file.run(params),
        }
    }
}
