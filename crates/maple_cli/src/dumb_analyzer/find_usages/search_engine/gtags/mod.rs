use std::io::BufRead;
use std::path::PathBuf;
use std::path::MAIN_SEPARATOR;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use filter::subprocess::Exec;

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

    pub fn create_tags(&self) -> Result<()> {
        std::fs::create_dir_all(&self.db_path)?;
        let exit_status = Exec::cmd("gtags")
            .env("GTAGSLABEL", "native-pygments")
            .env("GTAGSROOT", &self.project_root)
            .env("GTAGSDBPATH", &self.db_path)
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
        let exit_status = Exec::cmd("global")
            .env("GTAGSLABEL", "native-pygments")
            .env("GTAGSROOT", &self.project_root)
            .env("GTAGSDBPATH", &self.db_path)
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

        // Search
        // let query = "foo";
        // Ok(Exec::cmd("global").cwd(project_root).arg(foo).env("GTAGSROOTJ", project_root).env("GTAGSDBPATH", db_path))
    }

    /// Search definition tags.
    pub fn search_definitions(&self, keyword: &str) -> Result<impl Iterator<Item = TagInfo>> {
        let cmd = Exec::cmd("global")
            .env("GTAGSROOT", &self.project_root)
            .env("GTAGSDBPATH", &self.db_path)
            .cwd(&self.project_root)
            .arg(keyword)
            .arg("--result")
            .arg("ctags-x");

        let stdout = cmd.stream_stdout()?;

        // We usually have a decent amount of RAM nowdays.
        Ok(std::io::BufReader::with_capacity(8 * 1024 * 1024, stdout)
            .lines()
            .flatten()
            .filter_map(|s| s.parse::<TagInfo>().ok()))
    }

    /// Search reference tags.
    ///
    /// Reference means the reference to a symbol which has definitions.
    pub fn search_references(&self, keyword: &str) -> Result<impl Iterator<Item = TagInfo>> {
        let cmd = Exec::cmd("global")
            .env("GTAGSROOT", &self.project_root)
            .env("GTAGSDBPATH", &self.db_path)
            .cwd(&self.project_root)
            .arg(keyword)
            .arg("--reference")
            .arg("--result")
            .arg("ctags-x");

        let stdout = cmd.stream_stdout()?;

        // We usually have a decent amount of RAM nowdays.
        Ok(std::io::BufReader::with_capacity(8 * 1024 * 1024, stdout)
            .lines()
            .flatten()
            .filter_map(|s| s.parse::<TagInfo>().ok()))
    }
}

/*
pub fn search(project_root: PathBuf) -> Result<impl Iterator<Item = String>> {
    use std::io::BufRead;

    let gtags_searcher = GtagsSearcher::new(project_root);
    let stdout = gtags_searcher.create_or_update_tags()?.stream_stdout()?;

    // We usually have a decent amount of RAM nowdays.
    Ok(std::io::BufReader::with_capacity(8 * 1024 * 1024, stdout)
        .lines()
        .flatten())
}
*/

#[derive(Default, Debug)]
pub struct TagInfo {
    pub path: String,
    pub pattern: String,
    pub line: usize,
}

impl FromStr for TagInfo {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        pattern::parse_gtags(s)
            .map(|(line, path, pattern)| TagInfo {
                path: path.into(),
                pattern: pattern.into(),
                line,
            })
            .ok_or(())
    }
}

impl TagInfo {
    pub fn grep_format(
        &self,
        query: &str,
        kind: &str,
        ignorecase: bool,
    ) -> (String, Option<Vec<usize>>) {
        let mut formatted = format!("[{}]{}:{}:1:", kind, self.path, self.line);

        let found = if ignorecase {
            self.pattern.to_lowercase().find(&query.to_lowercase())
        } else {
            self.pattern.find(query)
        };

        let indices = if let Some(idx) = found {
            let start = formatted.len() + idx;
            let end = start + query.len();
            Some((start..end).into_iter().collect())
        } else {
            None
        };

        formatted.push_str(&self.pattern);

        (formatted, indices)
    }
}
