use crate::light_command::{set_current_dir, LightCommand};
use anyhow::Result;
use icon::IconPainter;
use std::path::PathBuf;
use std::process::Command;
use structopt::StructOpt;

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
        let mut cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.args(&["/C", &self.cmd]);
            cmd
        } else {
            let mut cmd = Command::new("bash");
            cmd.arg("-c").arg(&self.cmd);
            cmd
        };

        set_current_dir(&mut cmd, self.cmd_dir.clone());

        cmd
    }

    pub fn run(
        &self,
        number: Option<usize>,
        icon_painter: Option<IconPainter>,
        no_cache: bool,
    ) -> Result<()> {
        let mut exec_cmd = self.prepare_exec_cmd();

        let mut light_cmd = LightCommand::new(
            &mut exec_cmd,
            number,
            self.output.clone(),
            icon_painter,
            self.output_threshold,
        );

        let args = self
            .cmd
            .split_whitespace()
            .map(Into::into)
            .collect::<Vec<_>>();

        if !no_cache && self.cmd_dir.is_some() {
            light_cmd.try_cache_or_execute(&args, self.cmd_dir.clone().unwrap())
        } else {
            light_cmd.execute(&args)
        }
    }
}
