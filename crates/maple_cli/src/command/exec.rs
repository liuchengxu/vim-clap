use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use structopt::StructOpt;

use crate::app::Params;
use crate::process::{
    light::{set_current_dir, LightCommand},
    BaseCommand,
};

/// Execute the shell command
#[derive(StructOpt, Debug, Clone)]
pub struct Exec {
    /// Specify the system command to run.
    #[structopt(index = 1, short, long)]
    cmd: String,

    /// Specify the output file path when the output of command exceeds the threshold.
    #[structopt(long = "output")]
    output: Option<String>,

    /// Specify the working directory of CMD
    #[structopt(long = "cmd-dir", parse(from_os_str))]
    cmd_dir: Option<PathBuf>,

    /// Specify the threshold for writing the output of command to a tempfile.
    #[structopt(long = "output-threshold", default_value = "100000")]
    output_threshold: usize,
}

impl Exec {
    // This can work with the piped command, e.g., git ls-files | uniq.
    fn prepare_exec_cmd(&self) -> Command {
        let mut cmd = crate::process::std::build_command(&self.cmd);

        set_current_dir(&mut cmd, self.cmd_dir.clone());

        cmd
    }

    pub fn run(
        &self,
        Params {
            number,
            icon_painter,
            no_cache,
            ..
        }: Params,
    ) -> Result<()> {
        let mut exec_cmd = self.prepare_exec_cmd();

        let mut light_cmd = LightCommand::new(
            &mut exec_cmd,
            number,
            self.output.clone(),
            icon_painter,
            self.output_threshold,
        );

        let cwd = match &self.cmd_dir {
            Some(dir) => dir.clone(),
            None => std::env::current_dir()?,
        };

        let base_cmd = BaseCommand::new(self.cmd.clone(), cwd);

        light_cmd.try_cache_or_execute(base_cmd, no_cache)?.print();

        Ok(())
    }
}
