use anyhow::Result;
use structopt::StructOpt;

use super::SharedParams;
use crate::app::Params;
use crate::dumb_analyzer::{Filtering, TagsSearcher};
use crate::tools::ctags::TagsConfig;

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
    ignorecase: bool,
}

impl TagsFile {
    pub fn run(&self, _params: Params) -> Result<()> {
        let dir = self.shared.dir()?;

        let config = TagsConfig::new(
            self.shared.languages.clone(),
            &self.inner.kinds_all,
            &self.inner.fields,
            &self.inner.extras,
            &self.shared.files,
            &dir,
            self.shared.exclude_opt(),
        );

        let tags_searcher = TagsSearcher::new(config);

        if let Some(ref query) = self.query {
            let results = tags_searcher.search(query, Filtering::StartWith, self.force_generate)?;
            for line in results {
                println!("{:?}", line);
            }
        }

        Ok(())
    }
}
