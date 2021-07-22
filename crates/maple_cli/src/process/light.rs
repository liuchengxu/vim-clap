//! Wrapper of std `Command` with some optimization about the output.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output};

use anyhow::{anyhow, Result};

use icon::IconPainter;
use utility::{println_json, read_first_lines};

use crate::cache::Digest;
use crate::process::BaseCommand;

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

/// This struct represents all the info about the processed result of executed command.
#[derive(Debug, Clone)]
pub struct ExecutedInfo {
    /// The number of total output lines.
    pub total: usize,
    /// The lines that will be printed.
    pub lines: Vec<String>,
    /// If these info are from the cache.
    pub using_cache: bool,
    /// Optional temp cache file for the whole output.
    pub tempfile: Option<PathBuf>,
}

impl ExecutedInfo {
    /// Print the fields that are not empty to the terminal in json format.
    pub fn print(&self) {
        let Self {
            using_cache,
            tempfile,
            total,
            lines,
        } = self;

        if self.using_cache {
            if self.tempfile.is_some() {
                if self.lines.is_empty() {
                    println_json!(using_cache, tempfile, total);
                } else {
                    println_json!(using_cache, tempfile, total, lines);
                }
            } else {
                println_json!(total, lines);
            }
        } else if self.tempfile.is_some() {
            println_json!(tempfile, total, lines);
        } else {
            println_json!(total, lines);
        }
    }
}

const OUTPUT_THRESHOLD: usize = 100_000;

/// Environment for running LightCommand.
#[derive(Debug, Clone)]
pub struct CommandEnv {
    pub dir: Option<PathBuf>,
    pub total: usize,
    pub number: Option<usize>,
    pub output: Option<String>,
    pub icon_painter: Option<IconPainter>,
    pub output_threshold: usize,
}

impl Default for CommandEnv {
    fn default() -> Self {
        Self {
            dir: None,
            total: 0usize,
            number: None,
            output: None,
            icon_painter: None,
            output_threshold: OUTPUT_THRESHOLD,
        }
    }
}

impl CommandEnv {
    pub fn new(
        dir: Option<PathBuf>,
        number: Option<usize>,
        output: Option<String>,
        icon_painter: Option<IconPainter>,
        output_threshold: Option<usize>,
    ) -> Self {
        Self {
            dir,
            number,
            output,
            icon_painter,
            output_threshold: output_threshold.unwrap_or(OUTPUT_THRESHOLD),
            ..Default::default()
        }
    }

    #[inline]
    pub fn try_paint_icon<'b>(
        &self,
        top_n: impl std::iter::Iterator<Item = &'b str>,
    ) -> Vec<String> {
        if let Some(ref painter) = self.icon_painter {
            top_n.map(|x| painter.paint(x)).collect()
        } else {
            top_n.map(Into::into).collect()
        }
    }

    // TODO: add a cache upper bound?
    #[inline]
    pub fn should_create_cache(&self) -> bool {
        self.total > self.output_threshold
    }
}

/// Writes the whole stdout of LightCommand to a tempfile.
fn cache_stdout(base_cmd: &BaseCommand, cmd_stdout: &[u8]) -> Result<PathBuf> {
    let cached_filename = utility::calculate_hash(base_cmd);
    let cached_path = crate::utils::generate_cache_file_path(&cached_filename.to_string())?;

    File::create(&cached_path)?.write_all(cmd_stdout)?;

    Ok(cached_path)
}

/// A wrapper of std::process::Command for building cache, adding icon and minimalize the
/// throughput.
#[derive(Debug)]
pub struct LightCommand<'a> {
    cmd: &'a mut Command,
    env: CommandEnv,
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
            env: CommandEnv::new(None, number, output, icon_painter, Some(output_threshold)),
        }
    }

    /// Contructs LightCommand from grep opts.
    pub fn new_grep(
        cmd: &'a mut Command,
        dir: Option<PathBuf>,
        number: Option<usize>,
        icon_painter: Option<IconPainter>,
        output_threshold: Option<usize>,
    ) -> Self {
        Self {
            cmd,
            env: CommandEnv::new(dir, number, None, icon_painter, output_threshold),
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
    fn minimalize_job_overhead(&self, stdout: &[u8]) -> Result<ExecutedInfo> {
        if let Some(number) = self.env.number {
            // TODO: do not have to into String for whole stdout, find the nth index of newline.
            // &cmd_output.stdout[..nth_newline_index]
            let stdout_str = String::from_utf8_lossy(&stdout);
            let lines = self.try_prepend_icon(stdout_str.split('\n').take(number));
            let total = self.env.total;
            return Ok(ExecutedInfo {
                total,
                lines,
                using_cache: false,
                tempfile: None,
            });
        }
        Err(anyhow!(
            "--number is unspecified, no overhead minimalization"
        ))
    }

    fn try_prepend_icon<'b>(&self, top_n: impl std::iter::Iterator<Item = &'b str>) -> Vec<String> {
        let mut lines = self.env.try_paint_icon(top_n);
        trim_trailing(&mut lines);
        lines
    }

    /// Cache the stdout into a tempfile if the output threshold exceeds.
    fn create_cache(
        &self,
        base_cmd: BaseCommand,
        results_number: u64,
        cmd_stdout: &[u8],
    ) -> Result<(String, Option<PathBuf>)> {
        let cache_file = cache_stdout(&base_cmd, cmd_stdout)?;

        let digest = Digest::new(base_cmd, results_number, cache_file.clone());

        crate::cache::add_new_cache_digest(digest)?;

        Ok((
            // lines used for displaying directly.
            // &cmd_output.stdout[..nth_newline_index]
            String::from_utf8_lossy(cmd_stdout).into(),
            Some(cache_file),
        ))
    }

    fn handle_with_cache_digest(&self, digest: Digest) -> Result<ExecutedInfo> {
        let Digest {
            results_number,
            cached_path,
            ..
        } = digest;

        let lines = if let Ok(lines_iter) = read_first_lines(&cached_path, 100) {
            if let Some(ref painter) = self.env.icon_painter {
                lines_iter.map(|x| painter.paint(&x)).collect()
            } else {
                lines_iter.collect()
            }
        } else {
            Vec::new()
        };

        Ok(ExecutedInfo {
            using_cache: true,
            total: results_number as usize,
            tempfile: Some(cached_path),
            lines,
        })
    }

    /// Firstly try the cache given the command args and working dir.
    /// If the cache exists, returns the cache file directly.
    pub fn try_cache_or_execute(&mut self, base_cmd: BaseCommand) -> Result<ExecutedInfo> {
        if let Some(cache_digest) = base_cmd.cache_exists() {
            self.handle_with_cache_digest(cache_digest)
        } else {
            self.env.dir = Some(base_cmd.cwd.clone());
            self.execute(base_cmd)
        }
    }

    /// Execute the command directly and capture the output.
    ///
    /// Truncate the results to `self.number` if specified,
    /// otherwise print the total results or write them to
    /// a tempfile if they are more than `self.output_threshold`.
    /// This cached tempfile can be reused on the following runs.
    pub fn execute(&mut self, base_cmd: BaseCommand) -> Result<ExecutedInfo> {
        let cmd_output = self.output()?;
        let cmd_stdout = &cmd_output.stdout;

        self.env.total = bytecount::count(cmd_stdout, b'\n');

        if let Ok(executed_info) = self.minimalize_job_overhead(cmd_stdout) {
            return Ok(executed_info);
        }

        // Write the output to a tempfile if the lines are too many.
        let (stdout_str, cached_path) = if self.env.should_create_cache() {
            self.create_cache(base_cmd, self.env.total as u64, &cmd_stdout)?
        } else {
            (String::from_utf8_lossy(cmd_stdout).into(), None)
        };
        let lines = self.try_prepend_icon(stdout_str.split('\n'));

        Ok(ExecutedInfo {
            total: self.env.total,
            lines,
            tempfile: cached_path,
            using_cache: false,
        })
    }
}
