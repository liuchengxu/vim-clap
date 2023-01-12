use super::CtagsCommonArgs;
use crate::app::Args;
use anyhow::Result;
use clap::Parser;
use maple_core::find_usages::{CtagsSearcher, QueryType};
use maple_core::tools::ctags::TagsGenerator;

#[derive(Parser, Debug, Clone)]
struct TagsFileArgs {
    /// Same with the `--kinds-all` option of ctags.
    #[clap(long, default_value = "*")]
    kinds_all: String,

    /// Same with the `--fields` option of ctags.
    #[clap(long, default_value = "*")]
    fields: String,

    /// Same with the `--extras` option of ctags.
    #[clap(long, default_value = "*")]
    extras: String,
}

/// Manipulate the tags file.
#[derive(Parser, Debug, Clone)]
pub struct TagsFile {
    /// Arguments for creating tags file.
    #[clap(flatten)]
    t_args: TagsFileArgs,

    /// Ctags common arguments.
    #[clap(flatten)]
    c_args: CtagsCommonArgs,

    /// Search the tag matching the given query.
    #[clap(long)]
    query: Option<String>,

    /// Generate the tags file whether the tags file exists or not.
    #[clap(long)]
    force_generate: bool,
}

impl TagsFile {
    pub fn run(&self, _args: Args) -> Result<()> {
        let dir = self.c_args.dir()?;

        let exclude_opt = self.c_args.exclude_opt();
        let tags_generator = TagsGenerator::new(
            self.c_args.languages.clone(),
            &self.t_args.kinds_all,
            &self.t_args.fields,
            &self.t_args.extras,
            &self.c_args.files,
            &dir,
            &exclude_opt,
        );

        let tags_searcher = CtagsSearcher::new(tags_generator);

        if let Some(ref query) = self.query {
            let symbols =
                tags_searcher.search_symbols(query, QueryType::StartWith, self.force_generate)?;
            for symbol in symbols {
                println!("{symbol:?}");
            }
        }

        Ok(())
    }
}
