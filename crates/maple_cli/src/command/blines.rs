use anyhow::Result;
use clap::Parser;

use filter::Source;

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
                    .map(|(idx, item)| format!("{} {}", idx + 1, item))
                    .map(Into::into),
            ),
            params.into_filter_context(),
        )
    }
}
