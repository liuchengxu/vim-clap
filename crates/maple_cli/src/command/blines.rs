use std::io::BufRead;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use clap::Parser;

use filter::Source;
use matcher::Bonus;
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

        let filter_context = if let Some(extension) = self
            .input
            .extension()
            .and_then(|s| s.to_str().map(|s| s.to_string()))
        {
            params
                .into_filter_context()
                .bonuses(vec![Bonus::Language(extension.into())])
        } else {
            params.into_filter_context()
        };

        filter::dyn_run(
            &self.query,
            filter_context,
            Source::List(blines_item_stream()),
        )
    }
}
