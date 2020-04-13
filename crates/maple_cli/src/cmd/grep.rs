use crate::cmd::cache::{cache_exists, send_response_from_cache, SendResponse};
use crate::light_command::{set_current_dir, LightCommand};
use crate::utils::is_git_repo;
use crate::ContentFiltering;
use anyhow::Result;
use fuzzy_filter::{subprocess, Source};
use icon::IconPainter;
use std::path::PathBuf;
use std::process::Command;

const RG_ARGS: [&str; 7] = [
    "rg",
    "--column",
    "--line-number",
    "--no-heading",
    "--color=never",
    "--smart-case",
    "",
];

fn prepare_grep_and_args(cmd_str: &str, cmd_dir: Option<PathBuf>) -> (Command, Vec<&str>) {
    let args = cmd_str.split_whitespace().collect::<Vec<&str>>();

    let mut cmd = Command::new(args[0]);

    set_current_dir(&mut cmd, cmd_dir);

    (cmd, args)
}

/// Runs grep command and returns until its output stream is completed.
///
/// Write the output to the cache file if neccessary.
pub fn run(
    grep_cmd: String,
    grep_query: &str,
    glob: Option<&str>,
    cmd_dir: Option<PathBuf>,
    number: Option<usize>,
    icon_painter: Option<IconPainter>,
) -> Result<()> {
    let (mut cmd, mut args) = prepare_grep_and_args(&grep_cmd, cmd_dir);

    // We split out the grep opts and query in case of the possible escape issue of clap.
    args.push(grep_query);

    if let Some(g) = glob {
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

pub fn dyn_grep(
    grep_query: &str,
    cmd_dir: Option<PathBuf>,
    input: Option<PathBuf>,
    number: Option<usize>,
    icon_painter: Option<IconPainter>,
    no_cache: bool,
) -> Result<()> {
    let rg_cmd = "rg --column --line-number --no-heading --color=never --smart-case ''";

    let source: Source<std::iter::Empty<_>> = if let Some(tempfile) = input {
        Source::File(tempfile)
    } else if let Some(dir) = cmd_dir {
        if !no_cache {
            if let Ok((cached_file, _)) = cache_exists(&RG_ARGS, &dir) {
                let cached_source: Source<std::iter::Empty<_>> = Source::File(cached_file).into();
                return crate::cmd::filter::dyn_run(
                    grep_query,
                    cached_source,
                    None,
                    number,
                    None,
                    icon_painter,
                    ContentFiltering::GrepExcludeFilePath,
                );
            }
        }
        subprocess::Exec::shell(rg_cmd).cwd(dir).into()
    } else {
        subprocess::Exec::shell(rg_cmd).into()
    };

    crate::cmd::filter::dyn_run(
        grep_query,
        source,
        None,
        number,
        None,
        icon_painter,
        ContentFiltering::GrepExcludeFilePath,
    )
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
