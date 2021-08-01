use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

use filter::{matcher::Bonus, Source};

use crate::app::Params;

/// Fuzzy filter the current vim buffer given the query.
#[derive(StructOpt, Debug, Clone)]
pub struct Blines {
    /// Initial query string
    #[structopt(index = 1, long)]
    query: String,

    /// File path of current vim buffer.
    #[structopt(index = 2, long, parse(from_os_str))]
    input: PathBuf,
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
            vec![Bonus::None],
        )
    }
}
