use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use utility::is_git_repo;

use super::{rg_base_command, rg_command};
use crate::app::Params;
use crate::process::light::{CommandEnv, LightCommand};
use crate::utils::{send_response_from_cache, SendResponse};

#[derive(Parser, Debug, Clone)]
pub struct RipGrepForerunner {
    /// Specify the working directory of CMD
    #[clap(long = "cmd-dir", parse(from_os_str))]
    cmd_dir: Option<PathBuf>,

    /// Specify the threshold for writing the output of command to a tempfile.
    #[clap(long = "output-threshold", default_value = "30000")]
    output_threshold: usize,
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
                let base_cmd = rg_base_command(dir);
                if let Some((total, cache)) = base_cmd.cache_info() {
                    if total > 100000 {
                        send_response_from_cache(&cache, total as usize, SendResponse::Json, icon);
                        return Ok(());
                    }
                }
            }
        }

        if self.should_skip() {
            return Ok(());
        }

        let dir = match self.cmd_dir {
            Some(ref dir) => dir.clone(),
            None => std::env::current_dir()?,
        };

        let mut cmd = rg_command(&dir);

        let mut light_cmd = LightCommand::new(
            &mut cmd,
            CommandEnv::new(Some(dir.clone()), number, icon, Some(self.output_threshold)),
        );

        light_cmd.execute(rg_base_command(dir))?.print();

        Ok(())
    }
}
