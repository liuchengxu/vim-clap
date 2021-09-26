//! Wrapper of [`tokio::process::Command`].

use std::path::Path;

use anyhow::Result;
use tokio::process::Command;

/// Builds `Command` from a cmd string which can use pipe.
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

/// Unit type wrapper for [`tokio::process::Command`].
#[derive(Debug)]
pub struct TokioCommand(Command);

impl<T: AsRef<str>> From<T> for TokioCommand {
    fn from(cmd: T) -> Self {
        Self(build_command(cmd.as_ref()))
    }
}

impl TokioCommand {
    /// Constructs a new instance of [`TokioCommand`].
    pub fn new(cmd: impl AsRef<str>) -> Self {
        cmd.as_ref().into()
    }

    pub async fn lines(&mut self) -> Result<Vec<String>> {
        // Calling `output()` or `spawn().wait_with_output()` directly does not
        // work for Vim.
        // let output = self.0.spawn()?.wait_with_output().await?;
        //
        // TokioCommand works great for Neovim, but it seemingly has some issues with Vim due to
        // the stdout pipe stuffs, not sure the reason under the hood clearly, but StdCommand works
        // both for Neovim and Vim.
        let output = self.0.output().await?;

        super::process_output(output)
    }

    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
        self.0.current_dir(dir);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tokio_command() {
        let mut tokio_cmd: TokioCommand = format!(
            "ls {}",
            std::env::current_dir()
                .unwrap()
                .into_os_string()
                .into_string()
                .unwrap()
        )
        .into();
        assert_eq!(
            vec!["Cargo.toml", "benches", "src"],
            tokio_cmd.lines().await.unwrap()
        );
    }
}
