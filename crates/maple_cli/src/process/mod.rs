pub mod light;
pub mod rstd;
pub mod tokio;

use std::path::PathBuf;

use anyhow::Result;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::cache::{push_cache_digest, Digest};
use crate::datastore::CACHE_INFO_IN_MEMORY;

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

        push_cache_digest(digest)?;

        Ok(cache_file)
    }
}
