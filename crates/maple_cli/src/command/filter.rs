use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;

use filter::{subprocess::Exec, FilterContext, Source};
use matcher::{Bonus, ClapItem, FuzzyAlgorithm, MatchScope, Matcher};

use crate::app::Params;
use crate::paths::AbsPathBuf;

fn parse_bonus(s: &str) -> Bonus {
    if s.to_lowercase().as_str() == "filename" {
        Bonus::FileName
    } else {
        Bonus::None
    }
}

/// Execute the shell command
#[derive(Parser, Debug, Clone)]
pub struct Filter {
    /// Initial query string
    #[clap(index = 1, long)]
    query: String,

    /// Fuzzy matching algorithm
    #[clap(long, parse(from_str), default_value = "fzy")]
    algo: FuzzyAlgorithm,

    /// Shell command to produce the whole dataset that query is applied on.
    #[clap(long)]
    cmd: Option<String>,

    /// Working directory of shell command.
    #[clap(long)]
    cmd_dir: Option<String>,

    /// Recently opened file list for adding a bonus to the initial score.
    #[clap(long, parse(from_os_str))]
    recent_files: Option<PathBuf>,

    /// Read input from a file instead of stdin, only absolute file path is supported.
    #[clap(long)]
    input: Option<AbsPathBuf>,

    /// Apply the filter on the full line content or parial of it.
    #[clap(long, parse(from_str), default_value = "full")]
    match_scope: MatchScope,

    /// Add a bonus to the score of base matching algorithm.
    #[clap(long, parse(from_str = parse_bonus), default_value = "none")]
    bonus: Bonus,

    /// Synchronous filtering, returns until the input stream is complete.
    #[clap(long)]
    sync: bool,

    #[clap(long)]
    par_run: bool,
}

impl Filter {
    /// Firstly try building the Source from shell command, then the input file, finally reading the source from stdin.
    fn generate_source<I: Iterator<Item = Arc<dyn ClapItem>>>(&self) -> Source<I> {
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
                .unwrap_or(Source::<I>::Stdin)
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
                    .filter_map(|x| x.ok())
                    .collect();
                bonuses.push(Bonus::RecentFiles(lines.into()));
            }
        }

        bonuses
    }

    pub fn run(
        &self,
        Params {
            number,
            winwidth,
            icon,
            case_matching,
            ..
        }: Params,
    ) -> Result<()> {
        let matcher = Matcher::with_bonuses(self.get_bonuses(), self.algo, self.match_scope)
            .set_case_matching(case_matching);

        if self.sync {
            let ranked = filter::sync_run::<std::iter::Empty<_>>(
                &self.query,
                self.generate_source(),
                matcher,
            )?;

            printer::print_sync_filter_results(ranked, number, winwidth.unwrap_or(100), icon);
        } else {
            if self.par_run {
                filter::par_dyn_run::<std::iter::Empty<_>>(
                    &self.query,
                    self.generate_source(),
                    FilterContext::new(icon, number, winwidth, matcher),
                )?;
            } else {
                filter::dyn_run::<std::iter::Empty<_>>(
                    &self.query,
                    self.generate_source(),
                    FilterContext::new(icon, number, winwidth, matcher),
                )?;
            }
        }
        Ok(())
    }
}
