pub mod recursive;
pub mod tagsfile;

use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

use crate::app::Params;

/// Generate ctags recursively given the directory.
#[derive(StructOpt, Debug, Clone)]
pub(self) struct SharedParams {
    /// The directory for executing the ctags command.
    #[structopt(long, parse(from_os_str))]
    dir: PathBuf,

    /// Specify the language.
    #[structopt(long)]
    languages: Option<String>,

    /// Exclude files and directories matching 'pattern'.
    ///
    /// Will be translated into ctags' option: --exclude=pattern.
    #[structopt(long, default_value = ".git,*.json,node_modules,target,_build")]
    exclude: Vec<String>,
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
