mod app;
pub mod command;

/// Re-exports.
pub use app::{Args, RunCmd};

use icon::Icon;
use maple_core::cache::Digest;
use maple_core::process::{ExecInfo, ShellCommand};
use printer::{println_json, println_json_with_length};
use std::path::Path;
use std::process::Command as StdCommand;
use utils::{line_count, read_first_lines};

#[derive(Debug, Clone)]
#[allow(unused)]
pub enum SendResponse {
    Json,
    JsonWithContentLength,
}

/// Reads the first lines from cache file and send back the cached info.
pub fn send_response_from_cache(
    tempfile: &Path,
    total: usize,
    response_ty: SendResponse,
    icon: Icon,
) {
    let using_cache = true;
    if let Ok(iter) = read_first_lines(&tempfile, 100) {
        let lines: Vec<String> = if let Some(icon_kind) = icon.icon_kind() {
            iter.map(|x| icon_kind.add_icon_to_text(x)).collect()
        } else {
            iter.collect()
        };
        match response_ty {
            SendResponse::Json => println_json!(total, tempfile, using_cache, lines),
            SendResponse::JsonWithContentLength => {
                println_json_with_length!(total, tempfile, using_cache, lines)
            }
        }
    } else {
        match response_ty {
            SendResponse::Json => println_json!(total, tempfile, using_cache),
            SendResponse::JsonWithContentLength => {
                println_json_with_length!(total, tempfile, using_cache)
            }
        }
    }
}

/// A wrapper of `std::process::Command` that can reuse the cache if possible.
///
/// When no cache is usable, the command will be executed and the output will
/// be redirected to a cache file if there are too many items in the output.
///
/// NOTE: this was initially for the performance purpose, but no longer
/// necessory and has been retired.
#[derive(Debug)]
pub struct CacheableCommand<'a> {
    /// Ready to be executed and get the output.
    std_cmd: &'a mut StdCommand,
    /// Used to find and reuse the cache if any.
    shell_cmd: ShellCommand,
    number: usize,
    icon: Icon,
    output_threshold: usize,
}

impl<'a> CacheableCommand<'a> {
    /// Threshold for making a cache for the results.
    const OUTPUT_THRESHOLD: usize = 200_000;

    /// Contructs CacheableCommand from various common opts.
    pub fn new(
        std_cmd: &'a mut StdCommand,
        shell_cmd: ShellCommand,
        number: Option<usize>,
        icon: Icon,
        output_threshold: Option<usize>,
    ) -> Self {
        Self {
            std_cmd,
            shell_cmd,
            number: number.unwrap_or(100),
            icon,
            output_threshold: output_threshold.unwrap_or(Self::OUTPUT_THRESHOLD),
        }
    }

    /// Checks if the cache exists given `shell_cmd` and `no_cache` flag.
    /// If the cache exists, return the cached info, otherwise execute
    /// the command.
    pub fn try_cache_or_execute(&mut self, no_cache: bool) -> std::io::Result<ExecInfo> {
        if no_cache {
            self.execute()
        } else {
            self.shell_cmd
                .cache_digest()
                .map(|digest| self.exec_info_from_cache_digest(&digest))
                .unwrap_or_else(|| self.execute())
        }
    }

    fn exec_info_from_cache_digest(&self, digest: &Digest) -> std::io::Result<ExecInfo> {
        let Digest {
            total, cached_path, ..
        } = digest;

        let lines_iter = read_first_lines(&cached_path, self.number)?;
        let lines = if let Some(icon_kind) = self.icon.icon_kind() {
            lines_iter.map(|x| icon_kind.add_icon_to_text(x)).collect()
        } else {
            lines_iter.collect()
        };

        Ok(ExecInfo {
            using_cache: true,
            total: *total,
            tempfile: Some(cached_path.clone()),
            lines,
            icon_added: self.icon.enabled(),
        })
    }

    /// Execute the command and redirect the stdout to a file.
    pub fn execute(&mut self) -> std::io::Result<ExecInfo> {
        let cache_file_path = self.shell_cmd.cache_file_path()?;

        maple_core::process::write_stdout_to_file(self.std_cmd, &cache_file_path)?;

        let lines_iter = read_first_lines(&cache_file_path, 100)?;
        let lines = if let Some(icon_kind) = self.icon.icon_kind() {
            lines_iter.map(|x| icon_kind.add_icon_to_text(x)).collect()
        } else {
            lines_iter.collect()
        };

        let total = line_count(&cache_file_path)?;

        // Store the cache file if the total number of items exceeds the threshold, so that the
        // cache can be reused if the identical command is executed again.
        if total > self.output_threshold {
            let digest = Digest::new(self.shell_cmd.clone(), total, cache_file_path.clone());

            {
                let cache_info = maple_core::datastore::CACHE_INFO_IN_MEMORY.clone();
                let mut cache_info = cache_info.lock();
                cache_info.limited_push(digest)?;
            }
        }

        Ok(ExecInfo {
            using_cache: false,
            total,
            tempfile: Some(cache_file_path),
            lines,
            icon_added: self.icon.enabled(),
        })
    }
}
