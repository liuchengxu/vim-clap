use crate::app::Args;
use anyhow::Result;
use clap::Parser;
use filter::SequentialSource;
use maple_core::paths::AbsPathBuf;
use matcher::{Bonus, MatchResult};
use rayon::iter::ParallelBridge;
use std::borrow::Cow;
use std::io::BufRead;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use types::ClapItem;
use utils::display_width;

/// Fuzzy filter the current vim buffer given the query.
#[derive(Parser, Debug, Clone)]
pub struct Blines {
    /// Initial query string
    #[clap(index = 1)]
    query: String,

    /// File path of current vim buffer.
    #[clap(index = 2)]
    input: AbsPathBuf,

    #[clap(long)]
    par_run: bool,
}

#[derive(Debug)]
pub struct BlinesItem {
    pub raw: String,
    pub line_number: usize,
}

impl ClapItem for BlinesItem {
    fn raw_text(&self) -> &str {
        self.raw.as_str()
    }

    fn output_text(&self) -> Cow<'_, str> {
        format!("{} {}", self.line_number, self.raw).into()
    }

    fn match_result_callback(&self, match_result: MatchResult) -> MatchResult {
        let mut match_result = match_result;
        match_result.indices.iter_mut().for_each(|x| {
            *x += display_width(self.line_number) + 1;
        });
        match_result
    }
}

impl Blines {
    /// Looks for matches of `query` in lines of the current vim buffer.
    pub fn run(&self, args: Args) -> Result<()> {
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
                            let item: Arc<dyn ClapItem> = Arc::new(BlinesItem {
                                raw: line,
                                line_number: index + 1,
                            });

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
            args.into_filter_context()
                .bonuses(vec![Bonus::Language(extension.into())])
        } else {
            args.into_filter_context()
        };

        if self.par_run {
            filter::par_dyn_run_list(
                &self.query,
                filter_context,
                blines_item_stream().par_bridge(),
            );
        } else {
            filter::dyn_run(
                &self.query,
                filter_context,
                SequentialSource::Iterator(blines_item_stream()),
            )?;
        }

        Ok(())
    }
}
