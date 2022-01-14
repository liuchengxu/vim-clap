use std::path::PathBuf;
use std::path::MAIN_SEPARATOR;

use anyhow::Result;
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

    // export GTAGSLABEL=native-pygments
    fn create_or_update_tags(&self) -> Result<Exec> {
        // Generate or update the tags db.
        if self.db_path.exists() {
            // Update tags files increamentally.
            Ok(Exec::cmd("global")
                .env("GTAGSROOT", &self.project_root)
                .env("GTAGSDBPATH", &self.db_path)
                .cwd(&self.project_root)
                .arg("--update"))
        } else {
            std::fs::create_dir_all(&self.db_path)?;
            Ok(Exec::cmd("gtags")
                .env("GTAGSLABEL", "native-pygments")
                .cwd(&self.project_root)
                .arg(&self.db_path))
        }

        // Search
        // let query = "foo";
        // Ok(Exec::cmd("global").cwd(project_root).arg(foo).env("GTAGSROOTJ", project_root).env("GTAGSDBPATH", db_path))
    }

    /// Search definition tags.
    fn search_definitions(&self, keyword: &str) {
        let cmd = Exec::cmd("global")
            .env("GTAGSROOTJ", &self.project_root)
            .env("GTAGSDBPATH", &self.db_path)
            .cwd(&self.project_root)
            .arg(keyword)
            .arg("--result")
            .arg("ctags-x");
    }

    /// Search reference tags.
    ///
    /// Reference means the reference to a symbol which has definitions.
    fn search_references(&self, keyword: &str) {
        let cmd = Exec::cmd("global")
            .env("GTAGSROOTJ", &self.project_root)
            .env("GTAGSDBPATH", &self.db_path)
            .cwd(&self.project_root)
            .arg(keyword)
            .arg("--reference")
            .arg("--result")
            .arg("ctags-x");
    }
}

pub fn search(project_root: PathBuf) -> Result<impl Iterator<Item = String>> {
    use std::io::BufRead;

    let gtags_searcher = GtagsSearcher::new(project_root);
    let stdout = gtags_searcher.create_or_update_tags()?.stream_stdout()?;

    // We usually have a decent amount of RAM nowdays.
    Ok(std::io::BufReader::with_capacity(8 * 1024 * 1024, stdout)
        .lines()
        .flatten())
}

#[derive(Default, Debug)]
pub struct TagInfo {
    pub path: String,
    pub pattern: String,
    pub line: usize,
}

impl TryFrom<&str> for TagInfo {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        pattern::parse_gtags(s)
            .map(|(line, path, pattern)| TagInfo {
                path: path.into(),
                pattern: pattern.into(),
                line,
            })
            .ok_or(())
    }
}
