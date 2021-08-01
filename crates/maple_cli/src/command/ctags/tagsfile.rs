use structopt::StructOpt;

use anyhow::Result;

use super::SharedParams;

use crate::app::Params;

/// Generate ctags recursively given the directory.
#[derive(StructOpt, Debug, Clone)]
pub struct TagsFile {
    /// Shared parameters arouns ctags.
    #[structopt(flatten)]
    shared: SharedParams,
}

impl TagsFile {
    pub fn run(&self, params: Params) -> Result<()> {
        todo!()
    }
}
