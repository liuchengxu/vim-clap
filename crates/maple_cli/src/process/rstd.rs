//! Wrapper of std `Command`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Result};

/// Collect the output of command, exit directly if any error happened.
pub fn collect_stdout(cmd: &mut Command) -> Result<Vec<u8>> {
    let cmd_output = cmd.output()?;

    // vim-clap does not handle the stderr stream, we just pass the error info via stdout.
    if !cmd_output.status.success() && !cmd_output.stderr.is_empty() {
        let e = format!("{}", String::from_utf8_lossy(cmd_output.stderr.as_slice()));
        return Err(anyhow!(e));
    }

    Ok(cmd_output.stdout)
}

/// Builds `Command` from a cmd string which can use pipe.
///
/// This can work with the piped command, e.g., `git ls-files | uniq`.
pub fn build_command(inner_cmd: &str) -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(&["/C", inner_cmd]);
        cmd
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(inner_cmd);
        cmd
    }
}

/// Unit type wrapper for std command.
#[derive(Debug)]
pub struct StdCommand(Command);

impl From<&str> for StdCommand {
    fn from(cmd: &str) -> Self {
        Self(build_command(cmd))
    }
}

impl From<String> for StdCommand {
    fn from(cmd: String) -> Self {
        cmd.as_str().into()
    }
}

impl StdCommand {
    /// Constructs a `StdCommand` given the command String.
    #[allow(unused)]
    pub fn new(spawned_cmd: String) -> Self {
        Self(build_command(&spawned_cmd))
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

    /// Executes the command and consume the stdout as a stream of utf8 lines.
    fn _lines(&mut self) -> Result<Vec<String>> {
        let output = self.0.output()?;
        super::process_output(output)
    }

    pub fn lines(&mut self) -> Result<Vec<String>> {
        self._lines()
    }

    /// Executes the inner command and applies the predicate
    /// same with `filter_map` on each of stream line.
    pub fn filter_map_byte_line<B>(&mut self, f: impl FnMut(&[u8]) -> Option<B>) -> Result<Vec<B>> {
        let output = self.0.output()?;

        if !output.status.success() && !output.stderr.is_empty() {
            return Err(anyhow::anyhow!("an error occured: {:?}", output.stderr));
        }

        Ok(output.stdout.split(|x| x == &b'\n').filter_map(f).collect())
    }
}
