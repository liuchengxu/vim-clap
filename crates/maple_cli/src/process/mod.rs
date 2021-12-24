pub mod light;
pub mod rstd;
pub mod tokio;

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use self::rstd::StdCommand;

use crate::cache::{push_cache_digest, Digest};
use crate::datastore::CACHE_INFO_IN_MEMORY;

/// Converts [`std::process::Output`] to a Vec of String.
///
/// Remove the last line if it's empty.
pub fn process_output(output: std::process::Output) -> Result<Vec<String>> {
    if !output.status.success() && !output.stderr.is_empty() {
        return Err(anyhow!("Error in output: {:?}", output.stderr));
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

/// Wrapper type of [`StdCommand`].
#[derive(Debug)]
pub struct AsyncCommand(StdCommand);

impl AsyncCommand {
    pub fn new(command: impl AsRef<str>) -> Self {
        Self(command.as_ref().into())
    }

    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
        self.0.current_dir(dir);
        self
    }

    #[allow(unused)]
    pub async fn lines(&mut self) -> Result<Vec<String>> {
        self.0.lines()
    }

    pub fn stdout(&mut self) -> Result<Vec<u8>> {
        self.0.stdout()
    }
}

/// Shell command for executing with cache.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct BaseCommand {
    /// Raw shell command string.
    pub command: String,
    /// Working directory of command.
    ///
    /// The same command with different cwd normally has
    /// different results, thus we need to record the cwd too.
    pub cwd: PathBuf,
}

impl BaseCommand {
    /// Creates a new instance of [`BaseCommand`].
    pub fn new(command: String, cwd: PathBuf) -> Self {
        Self { command, cwd }
    }

    /// Returns the cache digest if the cache exists.
    pub fn cache_digest(&self) -> Option<Digest> {
        let mut info = CACHE_INFO_IN_MEMORY.lock();
        info.find_digest_usable(self)
    }

    pub fn cache_file(&self) -> Option<PathBuf> {
        let mut info = CACHE_INFO_IN_MEMORY.lock();
        info.find_digest_usable(self).map(|d| d.cached_path)
    }

    pub fn cache_info(&self) -> Option<(usize, PathBuf)> {
        let mut info = CACHE_INFO_IN_MEMORY.lock();
        info.find_digest_usable(self)
            .map(|d| (d.total, d.cached_path))
    }

    /// Executes and returns an value that implements `Read` trait.
    pub fn stream_stdout(&self) -> Result<impl std::io::Read> {
        let stdout_stream = filter::subprocess::Exec::shell(&self.command)
            .cwd(&self.cwd)
            .stream_stdout()?;

        Ok(stdout_stream)
    }

    /// Writes the whole stdout `cmd_stdout` to a cache file.
    fn write_stdout_to_disk(&self, cmd_stdout: &[u8]) -> Result<PathBuf> {
        use std::io::Write;

        let cached_filename = utility::calculate_hash(self).to_string();
        let cached_path = crate::utils::generate_cache_file_path(&cached_filename)?;

        std::fs::File::create(&cached_path)?.write_all(cmd_stdout)?;

        Ok(cached_path)
    }

    /// Caches the output into a tempfile and also writes the cache digest to the disk.
    pub fn create_cache(self, total: usize, cmd_stdout: &[u8]) -> Result<PathBuf> {
        let cache_file = self.write_stdout_to_disk(cmd_stdout)?;

        let digest = Digest::new(self, total, cache_file.clone());

        push_cache_digest(digest)?;

        Ok(cache_file)
    }
}
