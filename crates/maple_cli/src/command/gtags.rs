use anyhow::Result;
use structopt::StructOpt;

use filter::{matcher::Bonus, Source};

use crate::app::Params;
use crate::dumb_analyzer::GtagsSearcher;
use crate::paths::AbsPathBuf;

/// Fuzzy filter the current vim buffer given the query.
#[derive(StructOpt, Debug, Clone)]
pub struct Gtags {
    /// Initial query string
    #[structopt(index = 1, long)]
    query: String,

    /// File path of current vim buffer.
    #[structopt(index = 2, long)]
    cwd: AbsPathBuf,

    /// Search the reference tags.
    #[structopt(short, long)]
    reference: bool,
}

impl Gtags {
    /// Looks for matches of `query` in lines of the current vim buffer.
    pub fn run(&self, params: Params) -> Result<()> {
        let gtags_searcher = GtagsSearcher::new(self.cwd.as_ref().to_path_buf());

        gtags_searcher.create_or_update_tags()?;

        if self.reference {
            for line in gtags_searcher.search_references(&self.query)? {
                println!("{:?}", line.grep_format(&self.query, "refs", false));
            }
        } else {
            for line in gtags_searcher.search_definitions(&self.query)? {
                println!("{:?}", line.grep_format(&self.query, "defs", false));
            }
        }

        Ok(())
    }
}
