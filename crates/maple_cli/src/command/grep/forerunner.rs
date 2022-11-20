use super::{rg_command, rg_shell_command};
use crate::app::Params;
use crate::process::CacheableCommand;
use crate::utils::{send_response_from_cache, SendResponse};
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use utility::is_git_repo;

#[derive(Parser, Debug, Clone)]
pub struct RipGrepForerunner {
    /// Specify the working directory of CMD
    #[clap(long = "cmd-dir", parse(from_os_str))]
    cmd_dir: Option<PathBuf>,

    /// Specify the threshold for writing the output of command to a tempfile.
    #[clap(long = "output-threshold", default_value = "30000")]
    output_threshold: usize,

    /// Run without checking if cwd is a git repo.
    ///
    /// By default this command only works when cwd is a git repo.
    #[clap(long)]
    force_run: bool,
}

impl RipGrepForerunner {
    /// Skip the forerunner job if `cmd_dir` is not a git repo.
    ///
    /// Only spawn the forerunner job for git repo for now.
    fn should_skip(&self) -> bool {
        if let Some(ref dir) = self.cmd_dir {
            if !is_git_repo(dir) {
                return true;
            }
        } else if let Ok(dir) = std::env::current_dir() {
            if !is_git_repo(&dir) {
                return true;
            }
        }
        false
    }

    pub fn run(
        self,
        Params {
            number,
            icon,
            no_cache,
            ..
        }: Params,
    ) -> Result<()> {
        if !no_cache {
            if let Some(ref dir) = self.cmd_dir {
                let shell_cmd = rg_shell_command(dir);
                if let Some(digest) = shell_cmd.cache_digest() {
                    if digest.total > 100000 {
                        send_response_from_cache(
                            &digest.cached_path,
                            digest.total as usize,
                            SendResponse::Json,
                            icon,
                        );
                        return Ok(());
                    }
                }
            }
        }

        if !self.force_run && self.should_skip() {
            return Ok(());
        }

        let dir = match self.cmd_dir {
            Some(ref dir) => dir.clone(),
            None => std::env::current_dir()?,
        };

        let mut std_cmd = rg_command(&dir);
        CacheableCommand::new(
            &mut std_cmd,
            rg_shell_command(dir),
            number,
            icon,
            Some(self.output_threshold),
        )
        .execute()?
        .print();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Params;

    // TODO: fix and enable the test in CI
    #[test]
    #[ignore]
    fn ripgrep_forerunner_command_works() {
        let params = Params::parse_from(&["--no-cache", "--icon=Grep"]);

        let ripgrep_forerunner = RipGrepForerunner::parse_from(&[
            "",
            "--cmd-dir",
            std::env::current_dir()
                .unwrap()
                .into_os_string()
                .as_os_str()
                .to_str()
                .unwrap(),
            "--output-threshold",
            "100000",
            "--force-run",
        ]);

        ripgrep_forerunner
            .run(params)
            .expect("Failed to run command `ripgrep-forerunner`");
    }
}
