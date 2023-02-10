use crate::app::Args;
use anyhow::Result;
use clap::Parser;
use maple_core::find_usages::GtagsSearcher;
use maple_core::paths::AbsPathBuf;

/// Fuzzy filter the current vim buffer given the query.
#[derive(Parser, Debug, Clone)]
pub struct Gtags {
    /// Initial query string
    #[clap(index = 1)]
    query: String,

    /// File path of current vim buffer.
    #[clap(index = 2)]
    cwd: AbsPathBuf,

    /// Search the reference tags.
    #[clap(short, long)]
    reference: bool,
}

impl Gtags {
    pub fn run(&self, _args: Args) -> Result<()> {
        let gtags_searcher = GtagsSearcher::new(self.cwd.as_ref().to_path_buf());

        gtags_searcher.create_or_update_tags()?;

        if self.reference {
            for line in gtags_searcher.search_references(&self.query)? {
                println!("{:?}", line.grep_format_gtags("refs", &self.query, false));
            }
        } else {
            for line in gtags_searcher.search_definitions(&self.query)? {
                println!("{:?}", line.grep_format_gtags("defs", &self.query, false));
            }
        }

        Ok(())
    }
}
