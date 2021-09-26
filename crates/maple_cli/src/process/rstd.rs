//! Wrapper of [`std::process::Command`].

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Result};

/// Collect the output of command, exit directly if any error happened.
pub fn collect_stdout(cmd: &mut Command) -> Result<Vec<u8>> {
    let cmd_output = cmd.output()?;

    if !cmd_output.status.success() && !cmd_output.stderr.is_empty() {
        return Err(anyhow!("an error occured: {:?}", cmd_output.stderr));
    }

    Ok(cmd_output.stdout)
}

/// Builds [`std::process::Command`] from a cmd string which can use pipe.
///
/// This can work with the piped command, e.g., `git ls-files | uniq`.
fn build_command(shell_cmd: &str) -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(&["/C", shell_cmd]);
        cmd
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(shell_cmd);
        cmd
    }
}

/// Unit type wrapper for [`std::process::Command`].
#[derive(Debug)]
pub struct StdCommand(Command);

impl<T: AsRef<str>> From<T> for StdCommand {
    fn from(cmd: T) -> Self {
        Self(build_command(cmd.as_ref()))
    }
}

impl StdCommand {
    /// Constructs a new [`StdCommand`].
    pub fn new(cmd: impl AsRef<str>) -> Self {
        cmd.as_ref().into()
    }

    /// Sets the working directory for the inner `Command`.
    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
        // If `dir` is not a directory, use its parent as current dir.
        if dir.as_ref().is_dir() {
            self.0.current_dir(dir);
        } else {
            let mut cmd_dir: PathBuf = dir.as_ref().into();
            cmd_dir.pop();
            self.0.current_dir(cmd_dir);
        }

        self
    }

    /// Executes the command and collect the stdout in lines.
    pub fn lines(&mut self) -> Result<Vec<String>> {
        let output = self.0.output()?;
        super::process_output(output)
    }

    /// Returns the stdout of inner command.
    pub fn stdout(&mut self) -> Result<Vec<u8>> {
        collect_stdout(&mut self.0)
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.0.args(args);
        self
    }

    pub fn into_inner(self) -> Command {
        self.0
    }
}
