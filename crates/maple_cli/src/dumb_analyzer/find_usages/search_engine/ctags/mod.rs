use std::hash::Hash;
use std::path::{Path, PathBuf};

use anyhow::Result;
use filter::subprocess::{Exec, Redirection};

use super::{QueryType, Symbol};
use crate::tools::ctags::TagsConfig;

/// `readtags` powered searcher.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct CtagsSearcher<'a, P> {
    config: TagsConfig<'a, P>,
    tags_path: PathBuf,
}

impl<'a, P: AsRef<Path> + Hash> CtagsSearcher<'a, P> {
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

    fn build_exec(&self, query: &str, query_type: QueryType) -> Exec {
        // https://docs.ctags.io/en/latest/man/readtags.1.html#examples
        let cmd = Exec::cmd("readtags")
            .stderr(Redirection::Merge)
            .arg("--tag-file")
            .arg(&self.tags_path)
            .arg("-E")
            .arg("-ne");

        let cmd = if query.chars().all(char::is_lowercase) {
            cmd.arg("--icase-match")
        } else {
            cmd
        };

        match query_type {
            QueryType::StartWith => cmd.arg("--prefix-match").arg("-").arg(query),
            QueryType::Exact => cmd
                .arg("-Q")
                .arg(format!("(eq? (downcase $name) \"{}\")", query))
                .arg("-l"),
            QueryType::Contain => cmd
                .arg("-Q")
                .arg(format!("(substr? (downcase $name) \"{}\")", query))
                .arg("-l"),
            QueryType::Inherit => {
                todo!("Inherit")
            }
        }
    }

    pub fn search(
        &self,
        query: &str,
        query_type: QueryType,
        force_generate: bool,
    ) -> Result<impl Iterator<Item = Symbol>> {
        if force_generate || !self.tags_exists() {
            self.generate_tags()?;
        }

        let cmd = self.build_exec(query, query_type);

        Ok(crate::utils::lines(cmd)?
            .flatten()
            .filter_map(|s| Symbol::from_readtags(&s)))
    }
}
