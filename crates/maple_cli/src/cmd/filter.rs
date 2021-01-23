use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

use filter::{
    matcher::{Algo, Bonus, MatchType},
    subprocess, Source,
};
use icon::IconPainter;
use source_item::SourceItem;

fn parse_bonus(s: &str) -> Bonus {
    if s.to_lowercase().as_str() == "filename" {
        Bonus::FileName
    } else {
        Bonus::None
    }
}

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

    /// Recently opened file list for adding a bonus to the initial score.
    #[structopt(long, parse(from_os_str))]
    recent_files: Option<PathBuf>,

    /// Read input from a file instead of stdin, only absolute file path is supported.
    #[structopt(long, parse(from_os_str))]
    input: Option<PathBuf>,

    /// Apply the filter on the full line content or parial of it.
    #[structopt(short, long, possible_values = &MatchType::variants(), case_insensitive = true)]
    match_type: Option<MatchType>,

    /// Add a bonus to the score of base matching algorithm.
    #[structopt(short, long, parse(from_str = parse_bonus))]
    bonus: Option<Bonus>,

    /// Synchronous filtering, returns after the input stream is complete.
    #[structopt(short, long)]
    sync: bool,
}

impl Filter {
    /// Firstly try building the Source from shell command, then the input file, finally reading the source from stdin.
    fn generate_source<I: Iterator<Item = SourceItem>>(&self) -> Source<I> {
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

    fn get_bonuses(&self) -> Vec<Bonus> {
        use std::io::BufRead;

        let mut bonuses = vec![self.bonus.clone().unwrap_or_default()];
        if let Some(ref recent_files) = self.recent_files {
            // Ignore the error cases.
            if let Ok(file) = std::fs::File::open(recent_files) {
                let lines = std::io::BufReader::new(file)
                    .lines()
                    .filter_map(|x| x.ok())
                    .collect();
                bonuses.push(Bonus::RecentFiles(lines));
            }
        }

        bonuses
    }

    /// Returns the results until the input stream is complete.
    #[inline]
    fn sync_run(
        &self,
        number: Option<usize>,
        winwidth: Option<usize>,
        icon_painter: Option<IconPainter>,
    ) -> Result<()> {
        let ranked = filter::sync_run::<std::iter::Empty<_>>(
            &self.query,
            self.generate_source(),
            self.algo.clone().unwrap_or(Algo::Fzy),
            self.match_type.clone().unwrap_or(MatchType::Full),
            self.get_bonuses(),
        )?;

        printer::print_sync_filter_results(ranked, number, winwidth, icon_painter);

        Ok(())
    }

    #[inline]
    fn dyn_run(
        &self,
        number: Option<usize>,
        winwidth: Option<usize>,
        icon_painter: Option<IconPainter>,
    ) -> Result<()> {
        filter::dyn_run::<std::iter::Empty<_>>(
            &self.query,
            self.generate_source(),
            self.algo.clone(),
            number,
            winwidth,
            icon_painter,
            self.match_type.clone().unwrap_or(MatchType::Full),
            self.get_bonuses(),
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
