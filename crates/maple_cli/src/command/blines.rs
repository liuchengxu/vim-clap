use std::io::BufRead;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use clap::Parser;

use filter::Source;
use types::SourceItem;

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
        let source_file = std::fs::File::open(&self.input)?;

        let index = AtomicUsize::new(0);
        let blines_item_stream = || {
            std::io::BufReader::new(source_file)
                .lines()
                .filter_map(|x| {
                    x.ok().and_then(|line: String| {
                        let index = index.fetch_add(1, Ordering::SeqCst);
                        if line.trim().is_empty() {
                            None
                        } else {
                            let item = SourceItem::from(format!("{index} {line}"));
                            Some(item)
                        }
                    })
                })
        };

        filter::dyn_run(
            &self.query,
            Source::List(blines_item_stream()),
            params.into_filter_context(),
        )
    }
}
