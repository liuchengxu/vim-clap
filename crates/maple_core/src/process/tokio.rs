//! Wrapper of [`tokio::process::Command`].

use crate::process::process_output;
use std::path::Path;
use tokio::process::Command;

/// Executes the command and redirects the output to a file.
pub async fn write_stdout_to_file<P: AsRef<Path>>(
    cmd: &mut Command,
    output_file: P,
) -> std::io::Result<()> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(output_file)?;

    let exit_status = cmd.stdout(file).spawn()?.wait().await?;

    if exit_status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "Failed to execute the command: {cmd:?}, exit code: {:?}",
            exit_status.code()
        )))
    }
}

/// Builds `Command` from a cmd string which can use pipe.
///
/// This can work with the piped command, e.g., `git ls-files | uniq`.
pub fn shell_command(shell_cmd: impl AsRef<str>) -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", shell_cmd.as_ref()]);
        cmd
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(shell_cmd.as_ref());
        cmd
    }
}

/// Unit type wrapper for [`tokio::process::Command`].
#[derive(Debug)]
pub struct TokioCommand(Command);

impl From<std::process::Command> for TokioCommand {
    fn from(std_cmd: std::process::Command) -> Self {
        Self(std_cmd.into())
    }
}

impl TokioCommand {
    /// Constructs a new instance of [`TokioCommand`].
    pub fn new(shell_cmd: impl AsRef<str>) -> Self {
        Self(shell_command(shell_cmd))
    }

    pub async fn lines(&mut self) -> std::io::Result<Vec<String>> {
        // Calling `output()` or `spawn().wait_with_output()` directly does not
        // work for Vim.
        // let output = self.0.spawn()?.wait_with_output().await?;
        //
        // TokioCommand works great for Neovim, but it seemingly has some issues with Vim due to
        // the stdout pipe stuffs, not sure the reason under the hood clearly, but StdCommand works
        // both for Neovim and Vim.
        let output = self.0.output().await?;

        process_output(output)
    }

    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
        self.0.current_dir(dir);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[tokio::test]
    async fn test_tokio_command() {
        let shell_cmd = format!(
            "ls {}",
            std::env::current_dir()
                .unwrap()
                .into_os_string()
                .into_string()
                .unwrap()
        );
        let mut tokio_cmd = TokioCommand::new(shell_cmd);
        assert_eq!(
            vec!["Cargo.toml", "src"]
                .into_iter()
                .map(Into::into)
                .collect::<HashSet<String>>(),
            HashSet::from_iter(tokio_cmd.lines().await.unwrap().into_iter())
        );
    }
}
