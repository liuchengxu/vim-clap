use crate::app::Args;
use crate::CacheableCommand;
use anyhow::{Context, Result};
use clap::Parser;
use icon::Icon;
use maple_core::process::{shell_command, ShellCommand};
use maple_core::tools::rg::Match;
use rayon::prelude::*;
use std::convert::TryFrom;
use std::path::PathBuf;

/// Invoke the rg executable and return the raw output of rg.
///
/// The search result won't be shown until the stream is complete.
#[derive(Parser, Debug, Clone)]
pub struct LiveGrep {
    /// Specify the query string for GREP_CMD.
    #[clap(index = 1)]
    grep_query: String,

    /// Delegate to -g option of rg
    #[clap(long)]
    glob: Option<String>,

    /// Specify the grep command to run, normally rg will be used.
    ///
    /// Incase of clap can not reconginize such option: --cmd "rg --vimgrep ... "fn ul"".
    ///                                                       |-----------------|
    ///                                                   this can be seen as an option by mistake.
    #[clap(long, required_if_eq("sync", "true"))]
    grep_cmd: Option<String>,

    /// Specify the working directory of CMD
    #[clap(long, value_parser)]
    cmd_dir: Option<PathBuf>,

    /// Read input from a cached grep tempfile, only absolute file path is supported.
    #[clap(long, value_parser)]
    input: Option<PathBuf>,
}

impl LiveGrep {
    /// Runs grep command and returns until its output stream is completed.
    ///
    /// Write the output to the cache file if neccessary.
    pub fn run(
        &self,
        Args {
            number,
            winwidth,
            icon,
            ..
        }: Args,
    ) -> Result<()> {
        let mut grep_cmd = self
            .grep_cmd
            .clone()
            .context("--grep-cmd is required when --sync is on")?;

        if let Some(ref g) = self.glob {
            grep_cmd.push_str(" -g ");
            grep_cmd.push_str(g);
        }

        // Force using json format.
        grep_cmd.push_str(" --json ");
        grep_cmd.push_str(&self.grep_query);

        // currently vim-clap only supports rg.
        // Ref https://github.com/liuchengxu/vim-clap/pull/60
        grep_cmd.push_str(" .");

        // Shell command avoids https://github.com/liuchengxu/vim-clap/issues/595
        let mut std_cmd = shell_command(&grep_cmd);

        if let Some(ref dir) = self.cmd_dir {
            std_cmd.current_dir(dir);
        }

        let shell_cmd = ShellCommand::new(grep_cmd, std::env::current_dir()?);
        let execute_info =
            CacheableCommand::new(&mut std_cmd, shell_cmd, number, Default::default(), None)
                .execute()?;

        let enable_icon = !matches!(icon, Icon::Null);

        let (lines, indices): (Vec<String>, Vec<Vec<usize>>) = execute_info
            .lines
            .par_iter()
            .filter_map(|s| {
                Match::try_from(s.as_str())
                    .ok()
                    .map(|mat| mat.build_grep_line(enable_icon))
            })
            .unzip();

        let total = lines.len();

        let (lines, indices, truncated_map) = printer::truncate_grep_lines(
            lines,
            indices,
            winwidth.unwrap_or(80),
            if enable_icon { Some(2) } else { None },
        );

        if truncated_map.is_empty() {
            printer::println_json!(total, lines, indices);
        } else {
            let icon_added = enable_icon;
            printer::println_json!(total, lines, indices, truncated_map, icon_added);
        }

        Ok(())
    }
}
