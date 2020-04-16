use crate::cmd::cache::{cache_exists, send_response_from_cache, SendResponse};
use crate::light_command::{set_current_dir, LightCommand};
use crate::utils::is_git_repo;
use crate::ContentFiltering;
use anyhow::Result;
use fuzzy_filter::{subprocess::Exec, Source};
use icon::IconPainter;
use std::path::PathBuf;
use std::process::Command;
use structopt::StructOpt;

const RG_ARGS: [&str; 7] = [
    "rg",
    "--column",
    "--line-number",
    "--no-heading",
    "--color=never",
    "--smart-case",
    "",
];

const RG_EXEC_CMD: &str = "rg --column --line-number --no-heading --color=never --smart-case ''";

#[derive(StructOpt, Debug, Clone)]
pub struct Grep {
    /// Specify the grep command to run, normally rg will be used.
    ///
    /// Incase of clap can not reconginize such option: --cmd "rg --vimgrep ... "fn ul"".
    ///                                                       |-----------------|
    ///                                                   this can be seen as an option by mistake.
    #[structopt(index = 1, short, long)]
    grep_cmd: String,

    /// Specify the query string for GREP_CMD.
    #[structopt(index = 2, short, long)]
    grep_query: String,

    /// Delegate to -g option of rg
    #[structopt(short = "g", long = "glob")]
    glob: Option<String>,

    /// Specify the working directory of CMD
    #[structopt(long = "cmd-dir", parse(from_os_str))]
    cmd_dir: Option<PathBuf>,

    /// Synchronous filtering, returns after the input stream is complete.
    #[structopt(short, long)]
    sync: bool,

    /// Read input from a cached grep tempfile, only absolute file path is supported.
    #[structopt(long = "input", parse(from_os_str))]
    input: Option<PathBuf>,
}

fn prepare_grep_and_args(cmd_str: &str, cmd_dir: Option<PathBuf>) -> (Command, Vec<&str>) {
    let args = cmd_str.split_whitespace().collect::<Vec<&str>>();

    let mut cmd = Command::new(args[0]);

    set_current_dir(&mut cmd, cmd_dir);

    (cmd, args)
}

impl Grep {
    pub fn run(
        &self,
        number: Option<usize>,
        winwidth: Option<usize>,
        icon_painter: Option<IconPainter>,
        no_cache: bool,
    ) -> Result<()> {
        if self.sync {
            self.sync_run(number, icon_painter)?;
        } else {
            self.dyn_run(number, winwidth, icon_painter, no_cache)?;
        }
        Ok(())
    }

    /// Runs grep command and returns until its output stream is completed.
    ///
    /// Write the output to the cache file if neccessary.
    fn sync_run(&self, number: Option<usize>, icon_painter: Option<IconPainter>) -> Result<()> {
        let (mut cmd, mut args) = prepare_grep_and_args(&self.grep_cmd, self.cmd_dir.clone());

        // We split out the grep opts and query in case of the possible escape issue of clap.
        args.push(&self.grep_query);

        if let Some(ref g) = self.glob {
            args.push("-g");
            args.push(g);
        }

        // currently vim-clap only supports rg.
        // Ref https://github.com/liuchengxu/vim-clap/pull/60
        if cfg!(windows) {
            args.push(".");
        }

        cmd.args(&args[1..]);

        let mut light_cmd = LightCommand::new_grep(&mut cmd, None, number, icon_painter);

        light_cmd.execute(&args)?;

        Ok(())
    }

    /// Runs grep using the dyn filter.
    ///
    /// Firstly try using the cache.
    fn dyn_run(
        &self,
        number: Option<usize>,
        winwidth: Option<usize>,
        icon_painter: Option<IconPainter>,
        no_cache: bool,
    ) -> Result<()> {
        let do_dyn_filter = |source: Source<std::iter::Empty<_>>| {
            crate::cmd::filter::dyn_run(
                &self.grep_query,
                source,
                None,
                number,
                winwidth,
                icon_painter,
                ContentFiltering::GrepExcludeFilePath,
            )
        };

        let source: Source<std::iter::Empty<_>> = if let Some(ref tempfile) = self.input {
            Source::File(tempfile.clone())
        } else if let Some(ref dir) = self.cmd_dir {
            if !no_cache {
                if let Ok((cached_file, _)) = cache_exists(&RG_ARGS, dir) {
                    return do_dyn_filter(Source::File(cached_file));
                }
            }
            Exec::shell(RG_EXEC_CMD).cwd(dir).into()
        } else {
            Exec::shell(RG_EXEC_CMD).into()
        };

        do_dyn_filter(source)
    }
}

pub fn run_forerunner(
    cmd_dir: Option<PathBuf>,
    number: Option<usize>,
    icon_painter: Option<IconPainter>,
    no_cache: bool,
) -> Result<()> {
    if !no_cache {
        if let Some(ref dir) = cmd_dir {
            if let Ok((cache, total)) = cache_exists(&RG_ARGS, dir) {
                send_response_from_cache(
                    &cache,
                    total,
                    SendResponse::Json,
                    Some(IconPainter::Grep),
                );
                return Ok(());
            }
        }
    }

    let mut cmd = Command::new(RG_ARGS[0]);
    // Do not use --vimgrep here.
    cmd.args(&RG_ARGS[1..]);

    // Only spawn the forerunner job for git repo for now.
    if let Some(dir) = &cmd_dir {
        if !is_git_repo(dir) {
            return Ok(());
        }
    } else if let Ok(dir) = std::env::current_dir() {
        if !is_git_repo(&dir) {
            return Ok(());
        }
    }

    set_current_dir(&mut cmd, cmd_dir.clone());

    let mut light_cmd = LightCommand::new_grep(&mut cmd, cmd_dir, number, icon_painter);

    light_cmd.execute(&RG_ARGS)?;

    Ok(())
}
