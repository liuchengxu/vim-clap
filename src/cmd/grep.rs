use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;

use crate::light_command::{set_current_dir, LightCommand};

fn prepare_grep_and_args(cmd_str: &str, cmd_dir: Option<PathBuf>) -> (Command, Vec<String>) {
    let args = cmd_str
        .split_whitespace()
        .map(Into::into)
        .collect::<Vec<String>>();

    let mut cmd = Command::new(args[0].clone());

    set_current_dir(&mut cmd, cmd_dir);

    (cmd, args)
}

pub fn run(
    grep_cmd: String,
    grep_query: String,
    glob: Option<String>,
    cmd_dir: Option<PathBuf>,
    number: Option<usize>,
    enable_icon: bool,
) -> Result<()> {
    let (mut cmd, mut args) = prepare_grep_and_args(&grep_cmd, cmd_dir);

    // We split out the grep opts and query in case of the possible escape issue of clap.
    args.push(grep_query.to_string());

    if let Some(g) = glob {
        args.push("-g".into());
        args.push(g);
    }

    // currently vim-clap only supports rg.
    // Ref https://github.com/liuchengxu/vim-clap/pull/60
    if cfg!(windows) {
        args.push(".".into());
    }

    cmd.args(&args[1..]);

    let mut light_cmd = LightCommand::new_grep(&mut cmd, number, enable_icon);

    light_cmd.execute(&args)?;

    Ok(())
}

fn is_git_repo(dir: &mut PathBuf) -> bool {
    dir.push(".git");
    let is_git_repo = if dir.exists() { true } else { false };
    dir.pop();
    is_git_repo
}

pub fn run_forerunner(
    cmd_dir: Option<PathBuf>,
    number: Option<usize>,
    enable_icon: bool,
) -> Result<()> {
    let mut cmd = Command::new("rg");
    let args = [
        "--column",
        "--line-number",
        "--no-heading",
        "--color=never",
        "--smart-case",
        "",
    ];
    // Do not use --vimgrep here.
    cmd.args(&args);

    // Only spawn the forerunner job for git repo for now.
    if let Some(mut dir) = cmd_dir.clone() {
        if !is_git_repo(&mut dir) {
            return Ok(());
        }
    } else if let Ok(mut dir) = std::env::current_dir() {
        if !is_git_repo(&mut dir) {
            return Ok(());
        }
    }

    set_current_dir(&mut cmd, cmd_dir);

    let mut light_cmd = LightCommand::new_grep(&mut cmd, number, enable_icon);

    light_cmd.execute(&args.iter().map(|x| x.to_string()).collect::<Vec<_>>())?;

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
