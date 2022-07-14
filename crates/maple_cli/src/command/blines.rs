use std::sync::Arc;

use anyhow::Result;
use clap::Parser;

use filter::Source;
use matcher::ClapItem;

use crate::app::Params;
use crate::paths::AbsPathBuf;

/// Fuzzy filter the current vim buffer given the query.
#[derive(Parser, Debug, Clone)]
pub struct Blines {
    /// Initial query string
    #[clap(index = 1, long)]
    query: String,

    /// File path of current vim buffer.
    #[clap(index = 2, long)]
    input: AbsPathBuf,
}

impl Blines {
    /// Looks for matches of `query` in lines of the current vim buffer.
    pub fn run(&self, params: Params) -> Result<()> {
        filter::dyn_run(
            &self.query,
            Source::List(
                std::fs::read_to_string(&self.input)?
                    .lines()
                    .enumerate()
                    .map(|(idx, item)| {
                        let item: Arc<dyn ClapItem> = Arc::new(format!("{} {}", idx + 1, item));
                        item
                    })
                    .map(Into::into),
            ),
            params.into_filter_context(),
        )
    }
}
