use anyhow::Result;
use filter::{
    matcher::{Algo, LineSplitter},
    subprocess, Source,
};
use icon::IconPainter;
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

    /// Read input from a file instead of stdin, only absolute file path is supported.
    #[structopt(long = "input", parse(from_os_str))]
    input: Option<PathBuf>,

    /// Apply the filter on the full line content or parial of it.
    #[structopt(short, long, possible_values = &LineSplitter::variants(), case_insensitive = true)]
    line_splitter: Option<LineSplitter>,

    /// Synchronous filtering, returns after the input stream is complete.
    #[structopt(short, long)]
    sync: bool,
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
        let ranked = filter::sync_run::<std::iter::Empty<_>>(
            &self.query,
            self.generate_source(),
            self.algo.clone().unwrap_or(Algo::Fzy),
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
            self.line_splitter.clone().unwrap_or(LineSplitter::Full),
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
