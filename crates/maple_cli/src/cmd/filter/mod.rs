pub mod dynamic;

pub use dynamic::dyn_fuzzy_filter_and_rank as dyn_run;

use anyhow::Result;
use fuzzy_filter::{fuzzy_filter_and_rank, subprocess, Algo, ContentFiltering, Source};
use icon::{IconPainter, ICON_LEN};
use printer::truncate_long_matched_lines;
use std::collections::HashMap;
use std::path::PathBuf;
use structopt::StructOpt;

/// Execute the shell command
#[derive(StructOpt, Debug, Clone)]
pub struct Filter {
    /// Initial query string
    #[structopt(index = 1, short, long)]
    query: String,

    /// Filter algorithm
    #[structopt(short, long, possible_values = &Algo::variants(), case_insensitive = true)]
    algo: Option<Algo>,

    /// Shell command to produce the whole dataset that query is applied on.
    #[structopt(short, long)]
    cmd: Option<String>,

    /// Working directory of shell command.
    #[structopt(short, long)]
    cmd_dir: Option<String>,

    /// Synchronous filtering, returns after the input stream is complete.
    #[structopt(short, long)]
    sync: bool,

    /// Read input from a file instead of stdin, only absolute file path is supported.
    #[structopt(long = "input", parse(from_os_str))]
    input: Option<PathBuf>,

    /// Apply the filter on the full line content or parial of it.
    #[structopt(short, long, possible_values = &ContentFiltering::variants(), case_insensitive = true)]
    content_filtering: Option<ContentFiltering>,
}

impl Filter {
    /// Firstly try building the Source from shell command, then the input file, finally reading the source from stdin.
    fn generate_source<I: Iterator<Item = String>>(&self) -> Source<I> {
        if let Some(ref cmd_str) = self.cmd {
            if let Some(ref dir) = self.cmd_dir {
                subprocess::Exec::shell(cmd_str).cwd(dir).into()
            } else {
                subprocess::Exec::shell(cmd_str).into()
            }
        } else {
            self.input
                .clone()
                .map(Into::into)
                .unwrap_or(Source::<I>::Stdin)
        }
    }

    /// Returns the results until the input stream is complete.
    #[inline]
    fn sync_run(
        &self,
        number: Option<usize>,
        winwidth: Option<usize>,
        icon_painter: Option<IconPainter>,
    ) -> Result<()> {
        run::<std::iter::Empty<_>>(
            &self.query,
            self.generate_source(),
            self.algo.clone(),
            number,
            icon_painter,
            winwidth,
        )
    }

    #[inline]
    fn dyn_run(
        &self,
        number: Option<usize>,
        winwidth: Option<usize>,
        icon_painter: Option<IconPainter>,
    ) -> Result<()> {
        dyn_run::<std::iter::Empty<_>>(
            &self.query,
            self.generate_source(),
            self.algo.clone(),
            number,
            winwidth,
            icon_painter,
            self.content_filtering
                .clone()
                .unwrap_or(ContentFiltering::Full),
        )
    }

    pub fn run(
        &self,
        number: Option<usize>,
        winwidth: Option<usize>,
        icon_painter: Option<IconPainter>,
    ) -> Result<()> {
        if self.sync {
            self.sync_run(number, winwidth, icon_painter)?;
        } else {
            self.dyn_run(number, winwidth, icon_painter)?;
        }
        Ok(())
    }
}

/// Returns the info of the truncated top items ranked by the filtering score.
fn process_top_items<T>(
    top_size: usize,
    top_list: impl IntoIterator<Item = (String, T, Vec<usize>)>,
    winwidth: usize,
    icon_painter: Option<IconPainter>,
) -> (Vec<String>, Vec<Vec<usize>>, HashMap<String, String>) {
    let (truncated_lines, truncated_map) = truncate_long_matched_lines(top_list, winwidth, None);
    let mut lines = Vec::with_capacity(top_size);
    let mut indices = Vec::with_capacity(top_size);
    if let Some(painter) = icon_painter {
        for (text, _, idxs) in truncated_lines {
            let iconized = if let Some(origin_text) = truncated_map.get(&text) {
                format!("{} {}", painter.get_icon(origin_text), text)
            } else {
                painter.paint(&text)
            };
            lines.push(iconized);
            indices.push(idxs.into_iter().map(|x| x + ICON_LEN).collect());
        }
    } else {
        for (text, _, idxs) in truncated_lines {
            lines.push(text);
            indices.push(idxs);
        }
    }
    (lines, indices, truncated_map)
}

pub fn run<I: Iterator<Item = String>>(
    query: &str,
    source: Source<I>,
    algo: Option<Algo>,
    number: Option<usize>,
    icon_painter: Option<IconPainter>,
    winwidth: Option<usize>,
) -> Result<()> {
    let ranked = fuzzy_filter_and_rank(query, source, algo.unwrap_or(Algo::Fzy))?;

    if let Some(number) = number {
        let total = ranked.len();
        let (lines, indices, truncated_map) = process_top_items(
            number,
            ranked.into_iter().take(number),
            winwidth.unwrap_or(62),
            icon_painter,
        );
        if truncated_map.is_empty() {
            println_json!(total, lines, indices);
        } else {
            println_json!(total, lines, indices, truncated_map);
        }
    } else {
        for (text, _, indices) in ranked.iter() {
            println_json!(text, indices);
        }
    }

    Ok(())
}
