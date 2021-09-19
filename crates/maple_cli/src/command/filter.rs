use std::ops::Deref;
use std::path::PathBuf;

use anyhow::Result;
use structopt::StructOpt;

use filter::{
    matcher::{Bonus, FuzzyAlgorithm, MatchType},
    subprocess::Exec,
    FilterContext, Source,
};
use types::SourceItem;

use crate::{app::Params, paths::AbsPathBuf};

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
    #[structopt(index = 1, long)]
    query: String,

    /// Fuzzy matching algorithm
    #[structopt(long, parse(from_str), default_value = "fzy")]
    algo: FuzzyAlgorithm,

    /// Shell command to produce the whole dataset that query is applied on.
    #[structopt(long)]
    cmd: Option<String>,

    /// Working directory of shell command.
    #[structopt(long)]
    cmd_dir: Option<String>,

    /// Recently opened file list for adding a bonus to the initial score.
    #[structopt(long, parse(from_os_str))]
    recent_files: Option<PathBuf>,

    /// Read input from a file instead of stdin, only absolute file path is supported.
    #[structopt(long)]
    input: Option<AbsPathBuf>,

    /// Apply the filter on the full line content or parial of it.
    #[structopt(long, parse(from_str), default_value = "full")]
    match_type: MatchType,

    /// Add a bonus to the score of base matching algorithm.
    #[structopt(long, parse(from_str = parse_bonus), default_value = "none")]
    bonus: Bonus,

    /// Synchronous filtering, returns until the input stream is complete.
    #[structopt(long)]
    sync: bool,
}

impl Filter {
    /// Firstly try building the Source from shell command, then the input file, finally reading the source from stdin.
    fn generate_source<I: Iterator<Item = SourceItem>>(&self) -> Source<I> {
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
            ..
        }: Params,
    ) -> Result<()> {
        if self.sync {
            let ranked = filter::sync_run::<std::iter::Empty<_>>(
                &self.query,
                self.generate_source(),
                self.algo,
                self.match_type,
                self.get_bonuses(),
            )?;

            printer::print_sync_filter_results(ranked, number, winwidth.unwrap_or(100), icon);
        } else {
            filter::dyn_run::<std::iter::Empty<_>>(
                &self.query,
                self.generate_source(),
                FilterContext::new(self.algo, icon, number, winwidth, self.match_type),
                self.get_bonuses(),
            )?;
        }
        Ok(())
    }
}
