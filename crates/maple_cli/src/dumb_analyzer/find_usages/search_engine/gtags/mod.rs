use std::io::BufRead;
use std::path::{PathBuf, MAIN_SEPARATOR};

use anyhow::{anyhow, Result};
use filter::subprocess::Exec;

use super::TagInfo;
use crate::tools::gtags::GTAGS_DIR;

pub struct GtagsSearcher {
    pub project_root: PathBuf,
    pub db_path: PathBuf,
}

impl GtagsSearcher {
    pub fn new(project_root: PathBuf) -> Self {
        // Directory for GTAGS, GRTAGS, GPATH, e.g.,
        //
        // `~/.local/share/vimclap/gtags/project_root`
        let mut db_path = GTAGS_DIR.to_path_buf();
        db_path.push(
            project_root
                .display()
                .to_string()
                .replace(MAIN_SEPARATOR, "_"),
        );

        Self {
            project_root,
            db_path,
        }
    }

    /// Create or update the tags db.
    pub fn create_or_update_tags(&self) -> Result<()> {
        if self.db_path.exists() {
            self.update_tags()
        } else {
            self.create_tags()
        }
    }

    /// Contracts a `gtags` command with proper env variables.
    fn gtags(&self) -> Exec {
        Exec::cmd("gtags")
            .env("GTAGSROOT", &self.project_root)
            .env("GTAGSDBPATH", &self.db_path)
    }

    /// Contracts a `global` command with proper env variables.
    fn global(&self) -> Exec {
        Exec::cmd("global")
            .env("GTAGSROOT", &self.project_root)
            .env("GTAGSDBPATH", &self.db_path)
    }

    pub fn create_tags(&self) -> Result<()> {
        std::fs::create_dir_all(&self.db_path)?;
        let exit_status = self
            .gtags()
            .env("GTAGSLABEL", "native-pygments")
            .cwd(&self.project_root)
            .arg(&self.db_path)
            .join()?;
        if exit_status.success() {
            Ok(())
        } else {
            Err(anyhow!("Process for creating tags exited without success"))
        }
    }

    /// Update tags files increamentally.
    pub fn update_tags(&self) -> Result<()> {
        // GTAGSLABEL=native-pygments should be enabled.
        let exit_status = self
            .global()
            .env("GTAGSLABEL", "native-pygments")
            .cwd(&self.project_root)
            .arg("--update")
            .join()?;

        if exit_status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Gtags process for updating tags exited without success"
            ))
        }
    }

    /// Search definition tags exactly matching `keyword`.
    pub fn search_definitions(&self, keyword: &str) -> Result<impl Iterator<Item = TagInfo>> {
        let cmd = self
            .global()
            .cwd(&self.project_root)
            .arg(keyword)
            .arg("--result")
            .arg("ctags-x");

        execute(cmd)
    }

    /// Search reference tags exactly matching `keyword`.
    ///
    /// Reference means the reference to a symbol which has definitions.
    pub fn search_references(&self, keyword: &str) -> Result<impl Iterator<Item = TagInfo>> {
        let cmd = self
            .global()
            .cwd(&self.project_root)
            .arg(keyword)
            .arg("--reference")
            .arg("--result")
            .arg("ctags-x");

        execute(cmd)
    }

    // TODO prefix matching
    // GTAGSROOT=$(pwd) GTAGSDBPATH=/home/xlc/.local/share/vimclap/gtags/test/ global -g 'ru(.*)' --result=ctags-x
}

// Returns a stream of tag parsed from the gtags output.
fn execute(cmd: Exec) -> Result<impl Iterator<Item = TagInfo>> {
    let stdout = cmd.stream_stdout()?;

    // We usually have a decent amount of RAM nowdays.
    Ok(std::io::BufReader::with_capacity(8 * 1024 * 1024, stdout)
        .lines()
        .flatten()
        .filter_map(|s| TagInfo::from_gtags(&s)))
}
