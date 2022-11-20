mod forerunner;
mod grep_impl;
mod live_grep;

use crate::cache::Digest;
use crate::process::ShellCommand;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

pub use self::forerunner::RipGrepForerunner;
pub use self::grep_impl::Grep;
pub use self::live_grep::LiveGrep;

const RG_ARGS: &[&str] = &[
    "rg",
    "--column",
    "--line-number",
    "--no-heading",
    "--color=never",
    "--smart-case",
    "",
    ".",
];

// Ref https://github.com/liuchengxu/vim-clap/issues/533
// Now `.` is pushed to the end for all platforms due to https://github.com/liuchengxu/vim-clap/issues/711.
pub const RG_EXEC_CMD: &str =
    "rg --column --line-number --no-heading --color=never --smart-case '' .";

// Used for creating the cache in async context.
#[derive(Debug, Clone, Hash)]
pub struct RgTokioCommand {
    shell_cmd: ShellCommand,
}

impl RgTokioCommand {
    pub fn new(dir: PathBuf) -> Self {
        let shell_cmd = ShellCommand::new(RG_EXEC_CMD.into(), dir);
        Self { shell_cmd }
    }

    pub fn cache_digest(&self) -> Option<Digest> {
        self.shell_cmd.cache_digest()
    }

    pub async fn create_cache(self) -> Result<Digest> {
        let cache_file = self.shell_cmd.cache_file_path()?;

        let std_cmd = rg_command(&self.shell_cmd.cwd);
        let mut tokio_cmd = tokio::process::Command::from(std_cmd);
        crate::process::tokio::write_stdout_to_file(&mut tokio_cmd, &cache_file).await?;

        let digest = crate::cache::store_cache_digest(self.shell_cmd.clone(), cache_file)?;

        Ok(digest)
    }
}

pub fn rg_command<P: AsRef<Path>>(dir: P) -> Command {
    // Can not use StdCommand as it joins the args which does not work somehow.
    let mut cmd = Command::new(RG_ARGS[0]);
    // Do not use --vimgrep here.
    cmd.args(&RG_ARGS[1..]).current_dir(dir);
    cmd
}

#[inline]
pub fn rg_shell_command<P: AsRef<Path>>(dir: P) -> ShellCommand {
    ShellCommand::new(RG_EXEC_CMD.into(), PathBuf::from(dir.as_ref()))
}

pub fn refresh_cache(dir: impl AsRef<Path>) -> Result<Digest> {
    let shell_cmd = rg_shell_command(dir.as_ref());
    let cache_file_path = shell_cmd.cache_file_path()?;

    let mut cmd = rg_command(dir.as_ref());
    crate::process::write_stdout_to_file(&mut cmd, &cache_file_path)?;

    let digest = crate::cache::store_cache_digest(shell_cmd, cache_file_path)?;

    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::tokio::TokioCommand;
    use itertools::Itertools;
    use std::time::Instant;

    // 3X faster than the deprecated version.
    async fn create_cache_deprecated(dir: &Path) -> (usize, PathBuf) {
        let inner = ShellCommand::new(RG_EXEC_CMD.into(), dir.to_path_buf());

        let lines = TokioCommand::new(RG_EXEC_CMD)
            .current_dir(dir)
            .lines()
            .await
            .unwrap();

        let total = lines.len();
        let lines = lines.into_iter().join("\n");
        let cache_path = inner.write_cache(total, lines.as_bytes()).unwrap();

        (total, cache_path)
    }

    #[tokio::test]
    async fn test_create_grep_cache_async() {
        let dir = std::env::current_dir().unwrap();

        let now = Instant::now();
        let res = create_cache_deprecated(&dir).await;
        println!("Cache creation result(old): {res:?}");
        let elapsed = now.elapsed();
        println!("Elapsed: {:.3?}", elapsed);

        let now = Instant::now();
        let rg_cmd = RgTokioCommand::new(dir);
        let res = rg_cmd.create_cache().await.unwrap();
        println!(
            "Cache creation result(new): {:?}",
            (res.total, res.cached_path)
        );
        let elapsed = now.elapsed();
        println!("Elapsed: {:.3?}", elapsed);
    }
}
