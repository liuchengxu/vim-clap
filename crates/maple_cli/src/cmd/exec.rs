use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;

use crate::light_command::{set_current_dir, LightCommand};

// This can work with the piped command, e.g., git ls-files | uniq.
fn prepare_exec_cmd(cmd_str: &str, cmd_dir: Option<PathBuf>) -> Command {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(&["/C", cmd_str]);
        cmd
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(cmd_str);
        cmd
    };

    set_current_dir(&mut cmd, cmd_dir);

    cmd
}

pub fn run(
    cmd: String,
    output: Option<String>,
    output_threshold: usize,
    cmd_dir: Option<PathBuf>,
    number: Option<usize>,
    enable_icon: bool,
) -> Result<()> {
    let mut exec_cmd = prepare_exec_cmd(&cmd, cmd_dir.clone());

    let mut light_cmd = LightCommand::new(
        &mut exec_cmd,
        number,
        output,
        enable_icon,
        false,
        output_threshold,
    );

    let args = cmd.split_whitespace().map(Into::into).collect::<Vec<_>>();

    if let Some(dir) = cmd_dir {
        light_cmd.try_cache_or_execute(&args, dir)
    } else {
        light_cmd.execute(&args)
    }
}
