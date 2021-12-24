//! Wrapper of [`std::process::Command`] with some optimization about the output.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, Result};

use icon::Icon;
use utility::{println_json, read_first_lines};

use crate::cache::Digest;
use crate::process::BaseCommand;

/// Threshold for making a cache for the results.
const OUTPUT_THRESHOLD: usize = 50_000;

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

/// Environment for running [`LightCommand`].
#[derive(Debug, Clone)]
pub struct CommandEnv {
    pub dir: Option<PathBuf>,
    pub total: usize,
    pub number: Option<usize>,
    pub icon: Icon,
    pub output_threshold: usize,
}

impl Default for CommandEnv {
    fn default() -> Self {
        Self {
            dir: None,
            total: 0usize,
            number: None,
            icon: Default::default(),
            output_threshold: OUTPUT_THRESHOLD,
        }
    }
}

impl CommandEnv {
    pub fn new(
        dir: Option<PathBuf>,
        number: Option<usize>,
        icon: Icon,
        output_threshold: Option<usize>,
    ) -> Self {
        Self {
            dir,
            number,
            icon,
            output_threshold: output_threshold.unwrap_or(OUTPUT_THRESHOLD),
            ..Default::default()
        }
    }

    #[inline]
    pub fn try_paint_icon<'b>(
        &self,
        top_n: impl Iterator<Item = std::borrow::Cow<'b, str>>,
    ) -> Vec<String> {
        if let Some(painter) = self.icon.painter() {
            top_n.map(|x| painter.paint(x)).collect()
        } else {
            top_n.map(Into::into).collect()
        }
    }

    /// Returns true if the number of total results is larger than the output threshold.
    // TODO: add a cache upper bound?
    #[inline]
    pub fn should_create_cache(&self) -> bool {
        self.total > self.output_threshold
    }
}

/// A wrapper of std::process::Command with more more functions, including:
///
/// - Build cache for the larger results.
/// - Add an icon to the display line.
/// - Minimalize the throughput.
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
        icon: Icon,
        output_threshold: usize,
    ) -> Self {
        Self {
            cmd,
            env: CommandEnv::new(None, number, icon, Some(output_threshold)),
        }
    }

    /// Contructs LightCommand from grep opts.
    pub fn new_grep(
        cmd: &'a mut Command,
        dir: Option<PathBuf>,
        number: Option<usize>,
        icon: Icon,
        output_threshold: Option<usize>,
    ) -> Self {
        Self {
            cmd,
            env: CommandEnv::new(dir, number, icon, output_threshold),
        }
    }

    /// Collect the output of command, exit directly if any error happened.
    fn collect_stdout(&mut self) -> Result<Vec<u8>> {
        match crate::process::rstd::collect_stdout(&mut self.cmd) {
            Ok(stdout) => Ok(stdout),
            Err(e) => {
                // vim-clap does not handle the stderr stream, we just pass the error info via stdout.
                let error = e.to_string();
                println_json!(error);
                Err(e)
            }
        }
    }

    /// Normally we only care about the top N items and number of total results if it's not a
    /// forerunner job.
    fn minimalize_job_overhead(&self, stdout: &[u8]) -> Result<ExecutedInfo> {
        if let Some(number) = self.env.number {
            let lines = self.try_prepend_icon(
                stdout
                    .split(|x| x == &b'\n')
                    .map(|s| String::from_utf8_lossy(s))
                    .take(number),
            );
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

    fn try_prepend_icon<'b>(
        &self,
        top_n: impl std::iter::Iterator<Item = std::borrow::Cow<'b, str>>,
    ) -> Vec<String> {
        let mut lines = self.env.try_paint_icon(top_n);
        trim_trailing(&mut lines);
        lines
    }

    fn handle_cache_digest(&self, digest: &Digest) -> Result<ExecutedInfo> {
        let Digest {
            total, cached_path, ..
        } = digest;

        let lines = if let Ok(iter) = read_first_lines(&cached_path, 100) {
            if let Some(painter) = self.env.icon.painter() {
                iter.map(|x| painter.paint(&x)).collect()
            } else {
                iter.collect()
            }
        } else {
            Vec::new()
        };

        Ok(ExecutedInfo {
            using_cache: true,
            total: *total as usize,
            tempfile: Some(cached_path.clone()),
            lines,
        })
    }

    /// Checks if the cache exists given `base_cmd` and `no_cache` flag.
    /// If the cache exists, return the cached info, otherwise execute
    /// the command.
    pub fn try_cache_or_execute(
        &mut self,
        base_cmd: BaseCommand,
        no_cache: bool,
    ) -> Result<ExecutedInfo> {
        if !no_cache {
            if let Some(digest) = base_cmd.cache_digest() {
                self.handle_cache_digest(&digest)
            } else {
                self.execute(base_cmd)
            }
        } else {
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
        self.env.dir = Some(base_cmd.cwd.clone());

        let cmd_stdout = self.collect_stdout()?;

        self.env.total = bytecount::count(&cmd_stdout, b'\n');

        if let Ok(executed_info) = self.minimalize_job_overhead(&cmd_stdout) {
            return Ok(executed_info);
        }

        // Cache the output if there are too many lines.
        let cached_path = if self.env.should_create_cache() {
            let p = base_cmd.create_cache(self.env.total, &cmd_stdout)?;
            Some(p)
        } else {
            None
        };
        let lines = self.try_prepend_icon(
            cmd_stdout
                .split(|n| n == &b'\n')
                .map(|s| String::from_utf8_lossy(s)),
        );

        Ok(ExecutedInfo {
            total: self.env.total,
            lines,
            tempfile: cached_path,
            using_cache: false,
        })
    }
}
