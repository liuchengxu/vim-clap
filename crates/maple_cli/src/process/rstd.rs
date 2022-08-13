//! Wrapper of [`std::process::Command`].

use std::path::{Path, PathBuf};
use std::process::Command;

/// Executes the command and redirects the output to a file.
pub fn write_stdout_to_file<P: AsRef<Path>>(
    cmd: &mut Command,
    output_file: P,
) -> std::io::Result<()> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(output_file)?;

    let exit_status = cmd.stdout(file).spawn()?.wait()?;

    if exit_status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "Failed to execute the command: {cmd:?}, exit code: {:?}",
                exit_status.code()
            ),
        ))
    }
}

// TODO: make it configurable so that it can support powershell easier?
// https://github.com/liuchengxu/vim-clap/issues/640
/// Builds [`std::process::Command`] from a cmd string which can use pipe.
///
/// This can work with the piped command, e.g., `git ls-files | uniq`.
fn shell_command(shell_cmd: &str) -> Command {
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

impl From<Command> for StdCommand {
    fn from(cmd: Command) -> Self {
        Self(cmd)
    }
}

impl StdCommand {
    /// Constructs a new [`StdCommand`].
    pub fn new(shell_cmd: impl AsRef<str>) -> Self {
        Self(shell_command(shell_cmd.as_ref()))
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
    #[allow(unused)]
    pub fn lines(&mut self) -> std::io::Result<Vec<String>> {
        let output = self.0.output()?;
        super::process_output(output)
    }

    /// Returns the stdout of command, exit directly if any error happened.
    #[allow(unused)]
    pub fn stdout(&mut self) -> std::io::Result<Vec<u8>> {
        let cmd_output = self.0.output()?;

        if !cmd_output.status.success() && !cmd_output.stderr.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                String::from_utf8_lossy(&cmd_output.stderr),
            ));
        }

        Ok(cmd_output.stdout)
    }

    #[allow(unused)]
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
