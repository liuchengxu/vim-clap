use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use structopt::StructOpt;

use crate::app::Params;
use crate::process::{light::LightCommand, rstd::StdCommand, BaseCommand};

/// Execute the shell command
#[derive(StructOpt, Debug, Clone)]
pub struct Exec {
    /// Specify the system command to run.
    #[structopt(index = 1, long)]
    cmd: String,

    /// Specify the working directory of CMD
    #[structopt(long, parse(from_os_str))]
    cmd_dir: Option<PathBuf>,

    /// Specify the threshold for writing the output of command to a tempfile.
    #[structopt(long, default_value = "100000")]
    output_threshold: usize,
}

impl Exec {
    // This can work with the piped command, e.g., git ls-files | uniq.
    fn prepare_exec_cmd(&self) -> Command {
        let mut cmd = StdCommand::from(self.cmd.as_str());

        if let Some(ref cmd_dir) = self.cmd_dir {
            cmd.current_dir(cmd_dir);
        }

        cmd.into_inner()
    }

    pub fn run(
        &self,
        Params {
            number,
            icon,
            no_cache,
            ..
        }: Params,
    ) -> Result<()> {
        let mut exec_cmd = self.prepare_exec_cmd();

        let mut light_cmd = LightCommand::new(&mut exec_cmd, number, icon, self.output_threshold);

        let cwd = match &self.cmd_dir {
            Some(dir) => dir.clone(),
            None => std::env::current_dir()?,
        };

        let base_cmd = BaseCommand::new(self.cmd.clone(), cwd);

        light_cmd.try_cache_or_execute(base_cmd, no_cache)?.print();

        Ok(())
    }
}
