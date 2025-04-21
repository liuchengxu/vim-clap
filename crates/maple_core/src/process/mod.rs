pub mod subprocess;
pub mod tokio;

use crate::cache::{push_cache_digest, Digest};
use crate::datastore::{generate_cache_file_path, CACHE_INFO_IN_MEMORY};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

// TODO: make it configurable so that it can support powershell easier?
// https://github.com/liuchengxu/vim-clap/issues/640
/// Builds [`std::process::Command`] from a cmd string which can use pipe.
///
/// This can work with the piped command, e.g., `git ls-files | uniq`.
pub fn shell_command(shell_cmd: &str) -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", shell_cmd]);
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
        Err(std::io::Error::other(format!(
            "Failed to execute the command: {cmd:?}, exit code: {:?}",
            exit_status.code()
        )))
    }
}

/// Converts [`std::process::Output`] to a Vec of String.
///
/// Remove the last line if it's empty.
pub fn process_output(output: std::process::Output) -> std::io::Result<Vec<String>> {
    if !output.status.success() && !output.stderr.is_empty() {
        return Err(std::io::Error::other(String::from_utf8_lossy(
            &output.stderr,
        )));
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
    pub dir: PathBuf,
}

impl ShellCommand {
    /// Creates a new instance of [`ShellCommand`].
    pub fn new(command: String, dir: PathBuf) -> Self {
        Self { command, dir }
    }

    /// Returns the cache digest if the cache exists.
    pub fn cache_digest(&self) -> Option<Digest> {
        let mut info = CACHE_INFO_IN_MEMORY.lock();
        let maybe_usable_digest = info.lookup_usable_digest(self);
        if maybe_usable_digest.is_some() {
            info.store_cache_info_if_idle();
        }
        maybe_usable_digest
    }

    pub fn cache_file_path(&self) -> std::io::Result<PathBuf> {
        let cached_filename = utils::compute_hash(self);
        generate_cache_file_path(cached_filename.to_string())
    }

    // TODO: remove this.
    /// Caches the output into a tempfile and also writes the cache digest to the disk.
    pub fn write_cache(self, total: usize, cmd_stdout: &[u8]) -> std::io::Result<PathBuf> {
        use std::io::Write;

        let cache_filename = utils::compute_hash(&self);
        let cache_file = generate_cache_file_path(cache_filename.to_string())?;

        std::fs::File::create(&cache_file)?.write_all(cmd_stdout)?;

        let digest = Digest::new(self, total, cache_file.clone());

        push_cache_digest(digest);

        Ok(cache_file)
    }
}
