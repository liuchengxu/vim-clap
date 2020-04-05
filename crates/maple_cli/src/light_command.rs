use std::collections::hash_map::DefaultHasher;
use std::fs::{DirEntry, File};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::SystemTime;

use anyhow::{anyhow, Result};
use icon::{prepend_grep_icon, prepend_icon};

use crate::error::DummyError;
use crate::utils::read_first_lines;

/// Remove the last element if it's empty string.
#[inline]
fn trim_trailing(lines: &mut Vec<String>) {
    if let Some(last_line) = lines.last() {
        // " " len is 4.
        if last_line.is_empty() || last_line.len() == 4 {
            lines.remove(lines.len() - 1);
        }
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

// Formula: temp_dir + clap_cache + arg1_arg2_arg3 + hash(cmd_dir)
fn get_cache_dir(args: &[&str], cmd_dir: &PathBuf) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push("clap_cache");
    dir.push(args.join("_"));
    // TODO: use a readable cache cmd_dir name?
    dir.push(format!("{}", calculate_hash(&cmd_dir)));
    dir
}

/// Returns the cached entry given the cmd args and working dir.
fn get_cached_entry(args: &[&str], cmd_dir: &PathBuf) -> Result<DirEntry> {
    let cache_dir = get_cache_dir(args, &cmd_dir);
    if cache_dir.exists() {
        let mut entries = std::fs::read_dir(cache_dir)?;

        // TODO: get latest modifed cache file?
        if let Some(Ok(first_entry)) = entries.next() {
            return Ok(first_entry);
        }
    }

    Err(anyhow!(
        "Couldn't get the cached entry for {:?} {:?}",
        args,
        cmd_dir
    ))
}

pub fn set_current_dir(cmd: &mut Command, cmd_dir: Option<PathBuf>) {
    if let Some(cmd_dir) = cmd_dir {
        // If cmd_dir is not a directory, use its parent as current dir.
        if cmd_dir.is_dir() {
            cmd.current_dir(cmd_dir);
        } else {
            let mut cmd_dir = cmd_dir;
            cmd_dir.pop();
            cmd.current_dir(cmd_dir);
        }
    }
}

/// A wrapper of std::process::Command for building cache, adding icon and minimalize the
/// throughput.
#[derive(Debug)]
pub struct LightCommand<'a> {
    cmd: &'a mut Command,
    cmd_dir: Option<PathBuf>,
    total: usize,
    number: Option<usize>,
    output: Option<String>,
    enable_icon: bool,
    grep_enable_icon: bool,
    output_threshold: usize,
}

impl<'a> LightCommand<'a> {
    /// Contructs LightCommand from various common opts.
    pub fn new(
        cmd: &'a mut Command,
        number: Option<usize>,
        output: Option<String>,
        enable_icon: bool,
        grep_enable_icon: bool,
        output_threshold: usize,
    ) -> Self {
        Self {
            cmd,
            cmd_dir: None,
            number,
            total: 0usize,
            output,
            enable_icon,
            grep_enable_icon,
            output_threshold,
        }
    }

    /// Contructs LightCommand from grep opts.
    pub fn new_grep(cmd: &'a mut Command, number: Option<usize>, grep_enable_icon: bool) -> Self {
        Self {
            cmd,
            cmd_dir: None,
            number,
            total: 0usize,
            output: None,
            enable_icon: false,
            grep_enable_icon,
            output_threshold: 0usize,
        }
    }

    /// Collect the output of command, exit directly if any error happened.
    fn output(&mut self) -> Result<Output> {
        let cmd_output = self.cmd.output()?;

        // vim-clap does not handle the stderr stream, we just pass the error info via stdout.
        if !cmd_output.status.success() && !cmd_output.stderr.is_empty() {
            let error = format!("{}", String::from_utf8_lossy(&cmd_output.stderr));
            println_json!(error);
            std::process::exit(1);
        }

        Ok(cmd_output)
    }

    /// Normally we only care about the top N items and number of total results if it's not a
    /// forerunner job.
    fn minimalize_job_overhead(&self, stdout: &[u8]) -> Result<()> {
        if let Some(number) = self.number {
            // TODO: do not have to into String for whole stdout, find the nth index of newline.
            // &cmd_output.stdout[..nth_newline_index]
            let stdout_str = String::from_utf8_lossy(&stdout);
            let lines = self.try_prepend_icon(stdout_str.split('\n').take(number));
            let total = self.total;
            println_json!(total, lines);
            return Ok(());
        }
        Err(anyhow::Error::new(DummyError).context("No truncation"))
    }

    fn try_prepend_icon<'b>(&self, top_n: impl std::iter::Iterator<Item = &'b str>) -> Vec<String> {
        let mut lines = if self.grep_enable_icon {
            top_n.map(prepend_grep_icon).collect::<Vec<_>>()
        } else if self.enable_icon {
            top_n.map(prepend_icon).collect::<Vec<_>>()
        } else {
            top_n.map(Into::into).collect::<Vec<_>>()
        };
        trim_trailing(&mut lines);
        lines
    }

    fn tempfile(&self, args: &[&str]) -> Result<PathBuf> {
        if let Some(ref output) = self.output {
            Ok(output.into())
        } else {
            let mut dir = std::env::temp_dir();
            dir.push("clap_cache");
            dir.push(args.join("_"));
            if let Some(mut cmd_dir) = self.cmd_dir.clone() {
                dir.push(format!("{}", calculate_hash(&mut cmd_dir)));
            } else {
                dir.push("no_cmd_dir");
            }
            if !dir.exists() {
                std::fs::create_dir_all(&dir)?;
            }
            dir.push(format!(
                "{}_{}",
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)?
                    .as_secs(),
                self.total
            ));
            Ok(dir)
        }
    }

    /// Cache the stdout into a tempfile if the output threshold exceeds.
    fn try_cache(&self, cmd_stdout: &[u8], args: &[&str]) -> Result<(String, Option<PathBuf>)> {
        if self.total > self.output_threshold {
            let tempfile = self.tempfile(args)?;
            File::create(&tempfile)?.write_all(cmd_stdout)?;
            // FIXME find the nth newline index of stdout.
            // let _end = std::cmp::min(cmd_stdout.len(), 500);
            Ok((
                // lines used for displaying directly.
                // &cmd_output.stdout[..nth_newline_index]
                String::from_utf8_lossy(cmd_stdout).into(),
                Some(tempfile),
            ))
        } else {
            Ok((String::from_utf8_lossy(cmd_stdout).into(), None))
        }
    }

    fn prepend_icon_for_cached_lines(
        &self,
        lines_iter: impl Iterator<Item = String>,
    ) -> Vec<String> {
        if self.grep_enable_icon {
            lines_iter.map(|x| prepend_grep_icon(&x)).collect()
        } else if self.enable_icon {
            lines_iter.map(|x| prepend_icon(&x)).collect()
        } else {
            lines_iter.collect()
        }
    }

    /// Firstly try the cache given the command args and working dir.
    /// If the cache exists, returns the cache file directly.
    pub fn try_cache_or_execute(&mut self, args: &[&str], cmd_dir: PathBuf) -> Result<()> {
        if let Ok(cached_entry) = get_cached_entry(args, &cmd_dir) {
            let tempfile = cached_entry.path();
            if let Some(path_str) = cached_entry.file_name().to_str() {
                let info = path_str.split('_').collect::<Vec<_>>();
                if info.len() == 2 {
                    let total = info[1].parse::<u64>().unwrap();
                    let using_cache = true;
                    if let Ok(lines_iter) = read_first_lines(&tempfile, 100) {
                        let lines = self.prepend_icon_for_cached_lines(lines_iter);
                        println_json!(total, lines, tempfile, using_cache);
                    } else {
                        println_json!(total, tempfile, using_cache);
                    }
                    // TODO: refresh the cache or mark it as outdated?
                    return Ok(());
                }
            }
        }

        self.cmd_dir = Some(cmd_dir);

        self.execute(args)
    }

    /// Execute the command directly and capture the output.
    ///
    /// Truncate the results to `self.number` if specified,
    /// otherwise print the total results or write them to
    /// a tempfile if they are more than `self.output_threshold`.
    /// This cached tempfile can be reused on the following runs.
    pub fn execute(&mut self, args: &[&str]) -> Result<()> {
        let cmd_output = self.output()?;
        let cmd_stdout = &cmd_output.stdout;

        self.total = bytecount::count(cmd_stdout, b'\n');

        if self.minimalize_job_overhead(cmd_stdout).is_ok() {
            return Ok(());
        }

        // Write the output to a tempfile if the lines are too many.
        let (stdout_str, tempfile) = self.try_cache(&cmd_stdout, args)?;
        let lines = self.try_prepend_icon(stdout_str.split('\n'));
        let total = self.total;
        if let Some(tempfile) = tempfile {
            println_json!(total, lines, tempfile);
        } else {
            println_json!(total, lines);
        }

        Ok(())
    }
}

#[test]
fn test_trim_trailing() {
    use icon::DEFAULT_ICON;

    let empty_iconized_line = " ";

    assert_eq!(empty_iconized_line.len(), 4);
    assert!(empty_iconized_line.chars().next().unwrap() == DEFAULT_ICON);
}

#[test]
fn test_tmp_dir() {
    let mut dir = std::env::temp_dir();
    let args = ["fd", "--type", "f"];
    dir.push("clap_cache");
    dir.push(args.join("_"));
    std::fs::create_dir_all(&dir).unwrap();

    println!("dir:{:?}", dir);

    let cmd_dir: PathBuf = "/Users/xuliucheng".into();
    let hashed_cmd_dir = calculate_hash(&cmd_dir);

    println!("hashed dir:{:?}", hashed_cmd_dir);

    dir.push(format!("{}", hashed_cmd_dir));

    if dir.exists() {
        println!("exists");
        if let Ok(mut entries) = std::fs::read_dir(dir) {
            if let Some(Ok(first_entry)) = entries.next() {
                println!("first_entry: {:?}", first_entry.file_name());
            }
            // let filenames = entries.map(|x| x.unwrap().file_name());
            // for f in filenames {
            // println!("entries: {:?}", f);
            // }
        }
    } else {
        println!("does not exist, crate dir");
        std::fs::create_dir_all(&dir).unwrap();
    }
}

#[test]
fn test_lines() {
    use std::io::BufRead;
    let file: PathBuf = "/Users/xuliucheng/.spacevim".into();
    let file = File::open(&file).unwrap();

    // io::stdin().lock().lines().filter_map(|lines_iter| {
    // lines_iter.ok().and_then(|line| {
    // scorer(&line).map(|(score, indices)| (line, score, indices))
    // })
    // }),
    let lines = std::io::BufReader::new(file)
        .lines()
        .filter_map(|i| i.ok())
        .take(10)
        .collect::<Vec<_>>();
    println!("lines: {:?}", lines);
}
