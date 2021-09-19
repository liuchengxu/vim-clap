use super::*;

#[derive(StructOpt, Debug, Clone)]
pub struct RipGrepForerunner {
    /// Specify the working directory of CMD
    #[structopt(long = "cmd-dir", parse(from_os_str))]
    cmd_dir: Option<PathBuf>,

    /// Specify the threshold for writing the output of command to a tempfile.
    #[structopt(long = "output-threshold", default_value = "30000")]
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
                let base_cmd = BaseCommand::new(RG_EXEC_CMD.into(), dir.clone());
                if let Some((total, cache)) = base_cmd.cache_info() {
                    send_response_from_cache(&cache, total as usize, SendResponse::Json, icon);
                    return Ok(());
                }
            }
        }

        if self.should_skip() {
            return Ok(());
        }

        let mut std_cmd = StdCommand::new(RG_ARGS[0]);
        // Do not use --vimgrep here.
        std_cmd.args(&RG_ARGS[1..]);

        if let Some(ref dir) = self.cmd_dir {
            std_cmd.current_dir(dir);
        }

        let mut cmd = std_cmd.into_inner();

        let mut light_cmd = LightCommand::new_grep(
            &mut cmd,
            self.cmd_dir.clone(),
            number,
            icon,
            Some(self.output_threshold),
        );

        let cwd = match self.cmd_dir {
            Some(d) => d,
            None => std::env::current_dir()?,
        };
        let base_cmd = BaseCommand::new(RG_EXEC_CMD.into(), cwd);

        light_cmd.execute(base_cmd)?.print();

        Ok(())
    }
}
