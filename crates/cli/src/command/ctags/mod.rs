pub mod buffer_tags;
pub mod recursive_tags;
pub mod tags_file;

use crate::app::Args;
use anyhow::Result;
use clap::{Parser, Subcommand};
use itertools::Itertools;
use maple_core::paths::AbsPathBuf;
use maple_core::tools::ctags::EXCLUDE;
use std::path::PathBuf;

/// Generate ctags recursively given the directory.
#[derive(Parser, Debug, Clone)]
pub struct CtagsCommonArgs {
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
    )]
    exclude: String,

    /// Specify the input files.
    // - notify the tags update on demand.
    #[clap(long)]
    files: Vec<AbsPathBuf>,
}

impl CtagsCommonArgs {
    // TODO: remove this.
    pub fn exclude_opt(&self) -> String {
        self.exclude
            .split(',')
            .map(|x| format!("--exclude={x}"))
            .join(" ")
    }

    pub fn exclude_args(&self) -> Vec<String> {
        self.exclude
            .split(',')
            .map(|x| format!("--exclude={x}"))
            .collect()
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
    pub fn run(&self, args: Args) -> Result<()> {
        match self {
            Self::BufferTags(buffer_tags) => buffer_tags.run(args),
            Self::RecursiveTags(recursive_tags) => recursive_tags.run(args),
            Self::TagsFile(tags_file) => tags_file.run(args),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::RunCmd;

    #[test]
    fn test_ctags_command() {
        let run_cmd = RunCmd::parse_from(&[
            "",
            "ctags",
            "recursive-tags",
            "--query",
            "Query",
            "--exclude",
            ".git,target",
        ]);
        match run_cmd {
            RunCmd::Ctags(Ctags::RecursiveTags(rtags)) => {
                assert_eq!(
                    rtags.c_args.exclude_opt(),
                    "--exclude=.git --exclude=target".to_string(),
                )
            }
            _ => unreachable!(""),
        }
    }
}
