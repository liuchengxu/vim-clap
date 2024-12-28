use crate::app::Args;
use anyhow::Result;
use clap::Parser;
use filter::{filter_sequential, FilterContext, ParallelInputSource, SequentialSource};
use maple_core::paths::AbsPathBuf;
use matcher::{Bonus, FuzzyAlgorithm, MatchScope, MatcherBuilder};
use printer::Printer;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use subprocess::Exec;
use types::{ClapItem, MatchedItem};

fn parse_bonus(s: &str) -> Result<Bonus> {
    if s.to_lowercase().as_str() == "filename" {
        Ok(Bonus::FileName)
    } else {
        Ok(Bonus::None)
    }
}

/// Execute the shell command
#[derive(Parser, Debug, Clone)]
pub struct Filter {
    /// Initial query string
    #[clap(index = 1)]
    query: String,

    /// Fuzzy matching algorithm
    #[clap(long, value_parser, default_value = "fzy")]
    algo: FuzzyAlgorithm,

    /// Shell command to produce the whole dataset that query is applied on.
    #[clap(long)]
    cmd: Option<String>,

    /// Working directory of shell command.
    #[clap(long)]
    cmd_dir: Option<String>,

    /// Recently opened file list for adding a bonus to the initial score.
    #[clap(long, value_parser)]
    recent_files: Option<PathBuf>,

    /// Read input from a file instead of stdin, only absolute file path is supported.
    #[clap(long)]
    input: Option<AbsPathBuf>,

    /// Apply the filter on the full line content or parial of it.
    #[clap(long, value_parser, default_value = "full")]
    match_scope: MatchScope,

    /// Add a bonus to the score of base matching algorithm.
    #[clap(long, value_parser = parse_bonus, default_value = "none")]
    bonus: Bonus,

    /// Synchronous filtering, returns until the input stream is complete.
    #[clap(long)]
    sync: bool,

    #[clap(long)]
    par_run: bool,
}

/// Prints the results of filter::sync_run() to stdout.
fn print_sync_filter_results(
    matched_items: Vec<MatchedItem>,
    number: Option<usize>,
    printer: Printer,
) {
    if let Some(number) = number {
        let total_matched = matched_items.len();
        let mut matched_items = matched_items;
        matched_items.truncate(number);
        printer
            .to_display_lines(matched_items)
            .print_json(total_matched);
    } else {
        matched_items.iter().for_each(|matched_item| {
            let indices = &matched_item.indices;
            let text = matched_item.display_text();
            printer::println_json!(text, indices);
        });
    }
}

impl Filter {
    /// Firstly try building the Source from shell command, then the input file, finally reading the source from stdin.
    fn generate_source<I: Iterator<Item = Arc<dyn ClapItem>>>(&self) -> SequentialSource<I> {
        if let Some(ref cmd_str) = self.cmd {
            if let Some(ref dir) = self.cmd_dir {
                Exec::shell(cmd_str).cwd(dir).into()
            } else {
                Exec::shell(cmd_str).into()
            }
        } else {
            self.input
                .as_ref()
                .map(|i| i.deref().clone().into())
                .unwrap_or(SequentialSource::<I>::Stdin)
        }
    }

    fn generate_parallel_input_source(&self) -> ParallelInputSource {
        if let Some(ref cmd_str) = self.cmd {
            let exec = if let Some(ref dir) = self.cmd_dir {
                Exec::shell(cmd_str).cwd(dir)
            } else {
                Exec::shell(cmd_str)
            };
            ParallelInputSource::Exec(Box::new(exec))
        } else {
            let file = self
                .input
                .as_ref()
                .map(|i| i.deref().clone())
                .expect("Only File and Exec source can be parallel");
            ParallelInputSource::File(file)
        }
    }

    fn get_bonuses(&self) -> Vec<Bonus> {
        use std::io::BufRead;

        let mut bonuses = vec![self.bonus.clone()];
        if let Some(ref recent_files) = self.recent_files {
            // Ignore the error cases.
            if let Ok(file) = std::fs::File::open(recent_files) {
                let lines: Vec<String> = std::io::BufReader::new(file)
                    .lines()
                    .map_while(Result::ok)
                    .collect();
                bonuses.push(Bonus::RecentFiles(lines.into()));
            }
        }

        bonuses
    }

    pub fn run(
        &self,
        Args {
            number,
            winwidth,
            icon,
            case_matching,
            ..
        }: Args,
    ) -> Result<()> {
        let matcher_builder = MatcherBuilder::new()
            .bonuses(self.get_bonuses())
            .match_scope(self.match_scope)
            .fuzzy_algo(self.algo)
            .case_matching(case_matching);

        if self.sync {
            let ranked = filter_sequential(
                self.generate_source::<std::iter::Empty<_>>(),
                matcher_builder.build(self.query.as_str().into()),
            )?;

            let printer = Printer::new(winwidth.unwrap_or(100), icon);
            print_sync_filter_results(ranked, number, printer);
        } else if self.par_run {
            filter::par_dyn_run(
                &self.query,
                FilterContext::new(icon, number, winwidth, matcher_builder),
                self.generate_parallel_input_source(),
            )?;
        } else {
            filter::dyn_run::<std::iter::Empty<_>>(
                &self.query,
                FilterContext::new(icon, number, winwidth, matcher_builder),
                self.generate_source(),
            )?;
        }
        Ok(())
    }
}
