use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output};

use anyhow::Result;
use icon::IconPainter;

use crate::cmd::cache::CacheEntry;
use crate::error::DummyError;
use crate::utils::{get_cached_entry, read_first_lines, remove_dir_contents};

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
    icon_painter: Option<IconPainter>,
    output_threshold: usize,
}

impl<'a> LightCommand<'a> {
    /// Contructs LightCommand from various common opts.
    pub fn new(
        cmd: &'a mut Command,
        number: Option<usize>,
        output: Option<String>,
        icon_painter: Option<IconPainter>,
        output_threshold: usize,
    ) -> Self {
        Self {
            cmd,
            cmd_dir: None,
            number,
            total: 0usize,
            output,
            icon_painter,
            output_threshold,
        }
    }

    /// Contructs LightCommand from grep opts.
    pub fn new_grep(
        cmd: &'a mut Command,
        cmd_dir: Option<PathBuf>,
        number: Option<usize>,
        icon_painter: Option<IconPainter>,
    ) -> Self {
        Self {
            cmd,
            cmd_dir,
            number,
            total: 0usize,
            output: None,
            icon_painter,
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
        let mut lines = if let Some(ref painter) = self.icon_painter {
            top_n.map(|x| painter.paint(x)).collect::<Vec<_>>()
        } else {
            top_n.map(Into::into).collect::<Vec<_>>()
        };
        trim_trailing(&mut lines);
        lines
    }

    /// Cache the stdout into a tempfile if the output threshold exceeds.
    fn try_cache(&self, cmd_stdout: &[u8], args: &[&str]) -> Result<(String, Option<PathBuf>)> {
        // TODO: add a cache upper bound?
        if self.total > self.output_threshold {
            let tempfile = if let Some(ref output) = self.output {
                output.into()
            } else {
                CacheEntry::new(args, self.cmd_dir.clone(), self.total)?
            };

            // Remove the other outdated cache file if there are any.
            //
            // There should be only one cache file in parent_dir at this moment.
            if let Some(parent_dir) = tempfile.parent() {
                remove_dir_contents(&parent_dir.to_path_buf())?;
            }

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

    /// Firstly try the cache given the command args and working dir.
    /// If the cache exists, returns the cache file directly.
    pub fn try_cache_or_execute(&mut self, args: &[&str], cmd_dir: PathBuf) -> Result<()> {
        if let Ok(cached_entry) = get_cached_entry(args, &cmd_dir) {
            if let Ok(total) = CacheEntry::get_total(&cached_entry) {
                let using_cache = true;
                let tempfile = cached_entry.path();
                if let Ok(lines_iter) = read_first_lines(&tempfile, 100) {
                    let lines: Vec<String> = if let Some(ref painter) = self.icon_painter {
                        lines_iter.map(|x| painter.paint(&x)).collect()
                    } else {
                        lines_iter.collect()
                    };
                    println_json!(using_cache, total, tempfile, lines);
                } else {
                    println_json!(using_cache, total, tempfile);
                }
                // TODO: refresh the cache or mark it as outdated?
                return Ok(());
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
