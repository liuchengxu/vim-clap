pub mod tokio;

use crate::cache::{push_cache_digest, Digest};
use crate::datastore::CACHE_INFO_IN_MEMORY;
use anyhow::Result;
use icon::Icon;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use utility::{println_json, read_first_lines};

// TODO: make it configurable so that it can support powershell easier?
// https://github.com/liuchengxu/vim-clap/issues/640
/// Builds [`std::process::Command`] from a cmd string which can use pipe.
///
/// This can work with the piped command, e.g., `git ls-files | uniq`.
pub fn shell_command(shell_cmd: &str) -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(&["/C", shell_cmd]);
        cmd
    } else {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(shell_cmd);
        cmd
    }
}

/// Executes the command and redirects the output to a file.
pub fn write_stdout_to_file<P: AsRef<Path>>(
    cmd: &mut Command,
    output_file: P,
) -> std::io::Result<()> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(output_file)?;

    let exit_status = cmd.stdout(file).spawn()?.wait()?;

    if exit_status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "Failed to execute the command: {cmd:?}, exit code: {:?}",
                exit_status.code()
            ),
        ))
    }
}

/// Converts [`std::process::Output`] to a Vec of String.
///
/// Remove the last line if it's empty.
pub fn process_output(output: std::process::Output) -> std::io::Result<Vec<String>> {
    if !output.status.success() && !output.stderr.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            String::from_utf8_lossy(&output.stderr),
        ));
    }

    let mut lines = output
        .stdout
        .par_split(|x| x == &b'\n')
        .map(|s| String::from_utf8_lossy(s).to_string())
        .collect::<Vec<_>>();

    // Remove the last empty line.
    if lines.last().map(|s| s.is_empty()).unwrap_or(false) {
        lines.pop();
    }

    Ok(lines)
}

/// This type represents an identifier of an unique user-invoked shell command.
///
/// It's only used to determine the cache location for this command and should
/// never be used to be executed directly, in which case it's encouraged to use
/// `std::process::Command` or `subprocess::Exec:shell` instead.
/// Furthermore, it's recommended to execute the command directly instead of using
/// running in a shell like ['cmd', '/C'] due to some issue on Windows like [1].
///
/// [1] https://stackoverflow.com/questions/44757893/cmd-c-doesnt-work-in-rust-when-command-includes-spaces
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct ShellCommand {
    /// Raw shell command string.
    pub command: String,
    /// Working directory of command.
    ///
    /// The same command with different cwd normally has
    /// different results, thus we need to record the cwd too.
    pub cwd: PathBuf,
}

impl ShellCommand {
    /// Creates a new instance of [`ShellCommand`].
    pub fn new(command: String, cwd: PathBuf) -> Self {
        Self { command, cwd }
    }

    /// Returns the cache digest if the cache exists.
    pub fn cache_digest(&self) -> Option<Digest> {
        let mut info = CACHE_INFO_IN_MEMORY.lock();
        info.find_digest_usable(self)
    }

    pub fn cache_file_path(&self) -> std::io::Result<PathBuf> {
        let cached_filename = utility::calculate_hash(self);
        crate::utils::generate_cache_file_path(cached_filename.to_string())
    }

    // TODO: remove this.
    /// Caches the output into a tempfile and also writes the cache digest to the disk.
    pub fn write_cache(self, total: usize, cmd_stdout: &[u8]) -> Result<PathBuf> {
        use std::io::Write;

        let cache_filename = utility::calculate_hash(&self);
        let cache_file = crate::utils::generate_cache_file_path(cache_filename.to_string())?;

        std::fs::File::create(&cache_file)?.write_all(cmd_stdout)?;

        let digest = Digest::new(self, total, cache_file.clone());

        push_cache_digest(digest);

        Ok(cache_file)
    }
}

/// Threshold for making a cache for the results.
const OUTPUT_THRESHOLD: usize = 200_000;

/// This struct represents all the info about the processed result of executed command.
#[derive(Debug, Clone, Serialize)]
pub struct ExecInfo {
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

impl ExecInfo {
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

/// A wrapper of `std::process::Command` that can reuse the cache if possible.
///
/// When no cache is usable, the command will be executed and the output will be redirected to a
/// cache file if there are too many items in the output.
#[derive(Debug)]
pub struct CacheableCommand<'a> {
    /// Ready to be executed and get the output.
    std_cmd: &'a mut Command,
    /// Used to find and reuse the cache if any.
    shell_cmd: ShellCommand,
    number: usize,
    icon: Icon,
    output_threshold: usize,
}

impl<'a> CacheableCommand<'a> {
    /// Contructs CacheableCommand from various common opts.
    pub fn new(
        std_cmd: &'a mut Command,
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
            output_threshold: output_threshold.unwrap_or(OUTPUT_THRESHOLD),
        }
    }

    /// Checks if the cache exists given `shell_cmd` and `no_cache` flag.
    /// If the cache exists, return the cached info, otherwise execute
    /// the command.
    pub fn try_cache_or_execute(&mut self, no_cache: bool) -> Result<ExecInfo> {
        if no_cache {
            self.execute()
        } else {
            self.shell_cmd
                .cache_digest()
                .map(|digest| self.exec_info_from_cache_digest(&digest))
                .unwrap_or_else(|| self.execute())
        }
    }

    fn exec_info_from_cache_digest(&self, digest: &Digest) -> Result<ExecInfo> {
        let Digest {
            total, cached_path, ..
        } = digest;

        let lines_iter = read_first_lines(&cached_path, self.number)?;
        let lines = if let Some(icon_kind) = self.icon.icon_kind() {
            lines_iter.map(|x| icon_kind.add_icon_to_text(&x)).collect()
        } else {
            lines_iter.collect()
        };

        Ok(ExecInfo {
            using_cache: true,
            total: *total as usize,
            tempfile: Some(cached_path.clone()),
            lines,
            icon_added: self.icon.enabled(),
        })
    }

    /// Execute the command and redirect the stdout to a file.
    pub fn execute(&mut self) -> Result<ExecInfo> {
        let cache_file_path = self.shell_cmd.cache_file_path()?;

        write_stdout_to_file(self.std_cmd, &cache_file_path)?;

        let lines_iter = read_first_lines(&cache_file_path, 100)?;
        let lines = if let Some(icon_kind) = self.icon.icon_kind() {
            lines_iter.map(|x| icon_kind.add_icon_to_text(&x)).collect()
        } else {
            lines_iter.collect()
        };

        let total = crate::utils::count_lines(std::fs::File::open(&cache_file_path)?)?;

        // Store the cache file if the total number of items exceeds the threshold, so that the
        // cache can be reused if the identical command is executed again.
        if total > self.output_threshold {
            let digest = Digest::new(self.shell_cmd.clone(), total, cache_file_path.clone());

            {
                let cache_info = crate::datastore::CACHE_INFO_IN_MEMORY.clone();
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
