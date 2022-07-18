use std::borrow::Cow;
use std::io::BufRead;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;

use filter::ParSource;
use filter::Source;
use matcher::{Bonus, MatchResult};
use types::ClapItem;

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

    #[clap(long)]
    par_run: bool,
}

#[derive(Debug)]
struct BlinesItem {
    raw: String,
    line_number: usize,
}

impl ClapItem for BlinesItem {
    fn raw_text(&self) -> &str {
        self.raw.as_str()
    }

    fn display_text(&self) -> Cow<'_, str> {
        format!("{} {}", self.line_number, self.raw).into()
    }

    fn match_result_callback(&self, match_result: MatchResult) -> MatchResult {
        let mut match_result = match_result;
        match_result.indices.iter_mut().for_each(|x| {
            *x += crate::utils::display_width(self.line_number) + 1;
        });
        match_result
    }
}

impl Blines {
    /// Looks for matches of `query` in lines of the current vim buffer.
    pub fn run(&self, params: Params) -> Result<()> {
        let index = AtomicUsize::new(0);

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

        if self.par_run {
            let par_source = ParSource::File(self.input.clone().into());
            filter::par_dyn_run(&self.query, par_source, filter_context)
        } else {
            filter::dyn_run(
                &self.query,
                Source::List(
                    std::io::BufReader::new(std::fs::File::open(&self.input)?)
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
                        }),
                ),
                filter_context,
            )
        }
    }
}
