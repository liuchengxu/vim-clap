use anyhow::Result;
use filter::{matcher::LineSplitter, Source};
use std::path::PathBuf;
use structopt::StructOpt;

/// Fuzzy filter the current vim buffer given the query.
#[derive(StructOpt, Debug, Clone)]
pub struct Blines {
    /// Initial query string
    #[structopt(index = 1, short, long)]
    query: String,

    /// File path of current vim buffer.
    #[structopt(index = 2, short, long, parse(from_os_str))]
    input: PathBuf,
}

impl Blines {
    /// Looks for matches of `query` in lines of the current vim buffer.
    pub fn run(&self, number: Option<usize>, winwidth: Option<usize>) -> Result<()> {
        filter::dyn_run(
            &self.query,
            Source::List(
                std::fs::read_to_string(&self.input)?
                    .lines()
                    .enumerate()
                    .map(|(idx, item)| format!("{} {}", idx + 1, item)),
            ),
            None,
            number,
            winwidth,
            None,
            LineSplitter::Full,
        )
    }
}
