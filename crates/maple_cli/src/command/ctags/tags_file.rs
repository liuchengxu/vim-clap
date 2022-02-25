use anyhow::Result;
use clap::Parser;

use super::SharedParams;
use crate::app::Params;
use crate::find_usages::{CtagsSearcher, QueryType};
use crate::tools::ctags::TagsConfig;

#[derive(Parser, Debug, Clone)]
struct TagsFileParams {
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
    /// Params for creating tags file.
    #[clap(flatten)]
    inner: TagsFileParams,

    /// Shared parameters arouns ctags.
    #[clap(flatten)]
    shared: SharedParams,

    /// Search the tag matching the given query.
    #[clap(long)]
    query: Option<String>,

    /// Generate the tags file whether the tags file exists or not.
    #[clap(long)]
    force_generate: bool,
}

impl TagsFile {
    pub fn run(&self, _params: Params) -> Result<()> {
        let dir = self.shared.dir()?;

        let exclude_opt = self.shared.exclude_opt();
        let config = TagsConfig::new(
            self.shared.languages.clone(),
            &self.inner.kinds_all,
            &self.inner.fields,
            &self.inner.extras,
            &self.shared.files,
            &dir,
            &exclude_opt,
        );

        let tags_searcher = CtagsSearcher::new(config);

        if let Some(ref query) = self.query {
            let results = tags_searcher.search(query, QueryType::StartWith, self.force_generate)?;
            for line in results {
                println!("{:?}", line);
            }
        }

        Ok(())
    }
}
