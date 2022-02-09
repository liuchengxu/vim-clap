use std::hash::Hash;
use std::path::{Path, PathBuf};

use anyhow::Result;
use filter::subprocess::Exec;

use super::{SearchType, TagInfo};
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

    fn build_exec(&self, query: &str, search_type: SearchType) -> Exec {
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

        match search_type {
            SearchType::StartWith => cmd.arg("--prefix-match").arg("-").arg(query),
            SearchType::Exact => cmd
                .arg("-Q")
                .arg(format!("(eq? (downcase $name) \"{}\")", query))
                .arg("-l"),
            SearchType::Contain => cmd
                .arg("-Q")
                .arg(format!("(substr? (downcase $name) \"{}\")", query))
                .arg("-l"),
            SearchType::Inherit => {
                todo!("Inherit")
            }
        }
    }

    pub fn search(
        &self,
        query: &str,
        search_type: SearchType,
        force_generate: bool,
    ) -> Result<impl Iterator<Item = TagInfo>> {
        if force_generate || !self.tags_exists() {
            self.generate_tags()?;
        }

        let cmd = self.build_exec(query, search_type);

        Ok(crate::utils::lines(cmd)?
            .flatten()
            .filter_map(|s| TagInfo::from_readtags(&s)))
    }
}
