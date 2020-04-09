use crate::cmd::cache::CacheEntry;
use crate::light_command::{set_current_dir, LightCommand};
use crate::utils::{get_cached_entry, is_git_repo, read_first_lines};
use anyhow::{anyhow, Result};
use fuzzy_filter::{subprocess, Source};
use icon::prepend_grep_icon;
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

pub fn run(
    grep_cmd: String,
    grep_query: &str,
    glob: Option<&str>,
    cmd_dir: Option<PathBuf>,
    number: Option<usize>,
    enable_icon: bool,
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

    let mut light_cmd = LightCommand::new_grep(&mut cmd, None, number, enable_icon);

    light_cmd.execute(&args)?;

    Ok(())
}

fn cache_exists(
    args: &[&str],
    cmd_dir: &PathBuf,
    send_response: bool,
    with_length: bool,
) -> Result<PathBuf> {
    if let Ok(cached_entry) = get_cached_entry(args, cmd_dir) {
        if let Ok(total) = CacheEntry::get_total(&cached_entry) {
            let tempfile = cached_entry.path();
            if send_response {
                let using_cache = true;
                if let Ok(lines_iter) = read_first_lines(&tempfile, 100) {
                    let lines = lines_iter
                        .map(|x| prepend_grep_icon(&x))
                        .collect::<Vec<_>>();
                    if with_length {
                        print_json_with_length!(total, tempfile, using_cache, lines);
                    } else {
                        println_json!(total, tempfile, using_cache, lines);
                    }
                } else {
                    if with_length {
                        print_json_with_length!(total, tempfile, using_cache);
                    } else {
                        println_json!(total, tempfile, using_cache);
                    }
                }
            }
            // TODO: refresh the cache or mark it as outdated?
            return Ok(tempfile);
        }
    }
    Err(anyhow!(
        "Cache does not exist for: {:?} in {:?}",
        args,
        cmd_dir
    ))
}

pub fn dyn_grep(
    grep_query: &str,
    cmd_dir: Option<PathBuf>,
    input: Option<PathBuf>,
    number: Option<usize>,
    enable_icon: bool,
    no_cache: bool,
) -> Result<()> {
    let rg_cmd = "rg --column --line-number --no-heading --color=never --smart-case ''";

    let source: Source<std::iter::Empty<_>> = if let Some(tempfile) = input {
        Source::File(tempfile)
    } else if let Some(dir) = cmd_dir {
        if !no_cache {
            if let Ok(cached_file) = cache_exists(&RG_ARGS, &dir, false, true) {
                let cached_source: Source<std::iter::Empty<_>> = Source::File(cached_file).into();
                return crate::cmd::filter::dyn_run(
                    grep_query,
                    cached_source,
                    None,
                    number,
                    enable_icon,
                    None,
                    true,
                );
            }
        }
        subprocess::Exec::shell(rg_cmd).cwd(dir).into()
    } else {
        subprocess::Exec::shell(rg_cmd).into()
    };

    crate::cmd::filter::dyn_run(grep_query, source, None, number, enable_icon, None, true)
}

pub fn run_forerunner(
    cmd_dir: Option<PathBuf>,
    number: Option<usize>,
    enable_icon: bool,
    no_cache: bool,
) -> Result<()> {
    if !no_cache {
        if let Some(ref dir) = cmd_dir {
            if cache_exists(&RG_ARGS, dir, true, false).is_ok() {
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

    let mut light_cmd = LightCommand::new_grep(&mut cmd, cmd_dir, number, enable_icon);

    light_cmd.execute(&RG_ARGS)?;

    Ok(())
}

#[test]
fn test_git_repo() {
    let mut cmd_dir: PathBuf = "/Users/xuliucheng/.vim/plugged/vim-clap".into();
    cmd_dir.push(".git");
    if cmd_dir.exists() {
        println!("{:?} exists", cmd_dir);
    } else {
        println!("{:?} does not exist", cmd_dir);
    }
}
