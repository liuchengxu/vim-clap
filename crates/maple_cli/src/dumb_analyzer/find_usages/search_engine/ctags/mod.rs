use std::hash::Hash;
use std::path::{Path, PathBuf};

use anyhow::Result;
use filter::subprocess::Exec;

use super::TagInfo;
use crate::tools::ctags::TagsConfig;

#[derive(Clone, Debug)]
pub enum Filtering {
    StartWith,
    Contain,
    #[allow(unused)]
    Inherit,
}

/// `readtags` powered searcher.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct TagSearcher<'a, P> {
    config: TagsConfig<'a, P>,
    tags_path: PathBuf,
}

impl<'a, P: AsRef<Path> + Hash> TagSearcher<'a, P> {
    pub fn new(config: TagsConfig<'a, P>) -> Self {
        let tags_path = config.tags_path();
        Self { config, tags_path }
    }

    /// Returns `true` if the tags file already exists.
    pub fn tags_exists(&self) -> bool {
        self.tags_path.exists()
    }

    /// Generate the `tags` file.
    pub fn generate_tags(&self) -> Result<()> {
        self.config.generate_tags()
    }

    fn build_exec(&self, query: &str, filtering_type: Filtering) -> Exec {
        // https://docs.ctags.io/en/latest/man/readtags.1.html#examples
        let cmd = Exec::cmd("readtags")
            .arg("--tag-file")
            .arg(&self.tags_path)
            .arg("-E")
            .arg("-ne");

        let cmd = if query.chars().all(char::is_lowercase) {
            cmd.arg("--icase-match")
        } else {
            cmd
        };

        match filtering_type {
            Filtering::StartWith => cmd.arg("--prefix-match").arg("-").arg(query),
            Filtering::Contain => cmd
                .arg("-Q")
                .arg(format!("(substr? (downcase $name) \"{}\")", query))
                .arg("-l"),
            Filtering::Inherit => {
                todo!("Inherit")
            }
        }
    }

    pub fn search(
        &self,
        query: &str,
        filtering: Filtering,
        force_generate: bool,
    ) -> Result<impl Iterator<Item = TagInfo>> {
        use std::io::BufRead;

        if force_generate || !self.tags_exists() {
            self.generate_tags()?;
        }

        let stdout = self.build_exec(query, filtering).stream_stdout()?;

        // We usually have a decent amount of RAM nowdays.
        Ok(std::io::BufReader::with_capacity(8 * 1024 * 1024, stdout)
            .lines()
            .flatten()
            .filter_map(|s| TagInfo::from_ctags(&s)))
    }
}
