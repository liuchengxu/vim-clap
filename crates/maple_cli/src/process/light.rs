//! Wrapper of [`std::process::Command`] with some optimization about the output.

use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;

use icon::Icon;
use utility::{println_json, read_first_lines};

use crate::cache::Digest;
use crate::process::BaseCommand;

/// Threshold for making a cache for the results.
const OUTPUT_THRESHOLD: usize = 50_000;

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
    pub icon_added: bool,
}

impl ExecutedInfo {
    /// Print the fields that are not empty to the terminal in json format.
    pub fn print(&self) {
        let Self {
            using_cache,
            tempfile,
            total,
            lines,
            icon_added,
        } = self;

        if self.using_cache {
            if self.tempfile.is_some() {
                if self.lines.is_empty() {
                    println_json!(using_cache, tempfile, total, icon_added);
                } else {
                    println_json!(using_cache, tempfile, total, lines, icon_added);
                }
            } else {
                println_json!(total, lines);
            }
        } else if self.tempfile.is_some() {
            println_json!(tempfile, total, lines, icon_added);
        } else {
            println_json!(total, lines, icon_added);
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
    pub fn new(cmd: &'a mut Command, env: CommandEnv) -> Self {
        Self { cmd, env }
    }

    fn exec_info_from_cache_digest(&self, digest: &Digest) -> Result<ExecutedInfo> {
        let Digest {
            total, cached_path, ..
        } = digest;

        let lines_iter = read_first_lines(&cached_path, 100)?;
        let lines = if let Some(icon_kind) = self.env.icon.icon_kind() {
            lines_iter.map(|x| icon_kind.add_icon_to_text(&x)).collect()
        } else {
            lines_iter.collect()
        };

        Ok(ExecutedInfo {
            using_cache: true,
            total: *total as usize,
            tempfile: Some(cached_path.clone()),
            lines,
            icon_added: self.env.icon.enabled(),
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
        if no_cache {
            self.execute(base_cmd)
        } else {
            base_cmd
                .cache_digest()
                .map(|digest| self.exec_info_from_cache_digest(&digest))
                .unwrap_or_else(|| self.execute(base_cmd))
        }
    }

    /// Execute the command and redirect the stdout to a file.
    pub fn execute(&mut self, base_cmd: BaseCommand) -> Result<ExecutedInfo> {
        let cache_file_path = base_cmd.cache_file_path()?;

        crate::process::rstd::write_stdout_to_file(self.cmd, &cache_file_path)?;

        let lines_iter = read_first_lines(&cache_file_path, 100)?;
        let lines = if let Some(icon_kind) = self.env.icon.icon_kind() {
            lines_iter.map(|x| icon_kind.add_icon_to_text(&x)).collect()
        } else {
            lines_iter.collect()
        };

        let total = crate::utils::count_lines(std::fs::File::open(&cache_file_path)?)?;

        // Store the cache file if the total number of items exceeds the threshold, so that the
        // cache can be reused if the identical command is executed again.
        if total > self.env.output_threshold {
            let digest = Digest::new(base_cmd, total, cache_file_path.clone());

            {
                let cache_info = crate::datastore::CACHE_INFO_IN_MEMORY.clone();
                let mut cache_info = cache_info.lock();
                cache_info.limited_push(digest)?;
            }
        }

        Ok(ExecutedInfo {
            using_cache: false,
            total,
            tempfile: Some(cache_file_path),
            lines,
            icon_added: self.env.icon.enabled(),
        })
    }
}
