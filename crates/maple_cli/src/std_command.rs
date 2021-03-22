use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;

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
    pub fn new(spawned_cmd: String) -> Self {
        Self(build_command(&spawned_cmd))
    }

    /// Sets the working directory for the inner `Command`.
    pub fn current_dir(&mut self, cmd_dir: PathBuf) -> &mut Self {
        // If cmd_dir is not a directory, use its parent as current dir.
        if cmd_dir.is_dir() {
            self.0.current_dir(cmd_dir);
        } else {
            let mut cmd_dir = cmd_dir;
            cmd_dir.pop();
            self.0.current_dir(cmd_dir);
        }

        self
    }

    pub fn lines(&mut self) -> Result<Vec<String>> {
        let output = self.0.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.split('\n').map(Into::into).collect())
    }
}
