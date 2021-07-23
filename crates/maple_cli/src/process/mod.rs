pub mod light;
pub mod std;
pub mod tokio;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use self::std::StdCommand;

use crate::cache::{Digest, CACHE_INFO_IN_MEMORY};

/// Converts [`std::process::Output`] to a Vec of String.
///
/// Remove the last line if it's empty.
pub fn process_output(output: ::std::process::Output) -> Result<Vec<String>> {
    if !output.status.success() && !output.stderr.is_empty() {
        return Err(anyhow::anyhow!("an error occured: {:?}", output.stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut lines = stdout.split('\n').map(Into::into).collect::<Vec<String>>();

    // Remove the last empty line.
    if lines.last().map(|s| s.is_empty()).unwrap_or(false) {
        lines.pop();
    }

    Ok(lines)
}

/// Wrapper type of `StdCommand`.
#[derive(Debug)]
pub struct AsyncCommand(StdCommand);

impl AsyncCommand {
    pub fn new(command: String) -> Self {
        Self(command.into())
    }

    pub fn current_dir<P: AsRef<::std::path::Path>>(&mut self, dir: P) -> &mut Self {
        self.0.current_dir(dir);
        self
    }

    pub async fn lines(&mut self) -> Result<Vec<String>> {
        self.0.lines().await
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct BaseCommand {
    /// Raw shell command string.
    pub command: String,
    /// Working directory of command.
    ///
    /// The same command with different cwd normally has
    /// different results, thus we need to record the cwd too.
    pub cwd: ::std::path::PathBuf,
}

impl BaseCommand {
    /// Creates a new instance of [`BaseCommand`].
    pub fn new(command: String, cwd: ::std::path::PathBuf) -> Self {
        Self { command, cwd }
    }

    /// Returns the cache digest if the cache exists.
    pub fn cache_digest(&self) -> Option<Digest> {
        let info = CACHE_INFO_IN_MEMORY.lock().unwrap();
        info.find_digest_usable(self).cloned()
    }

    pub fn cache_file(&self) -> Option<::std::path::PathBuf> {
        let info = CACHE_INFO_IN_MEMORY.lock().unwrap();
        info.find_digest_usable(self).map(|d| d.cached_path.clone())
    }

    pub fn cached_info(&self) -> Option<(usize, ::std::path::PathBuf)> {
        let info = CACHE_INFO_IN_MEMORY.lock().unwrap();
        info.find_digest_usable(&self)
            .map(|d| (d.total, d.cached_path.clone()))
    }
}
