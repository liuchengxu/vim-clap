pub mod kinds;

use super::{QueryType, Symbol};
use crate::find_usages::AddressableUsage;
use crate::tools::ctags::TagsGenerator;
use crate::utils::UsageMatcher;
use anyhow::Result;
use itertools::Itertools;
use rayon::prelude::*;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use subprocess::{Exec, Redirection};

/// `readtags` powered searcher.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct CtagsSearcher<'a, P> {
    tags_path: PathBuf,
    tags_generator: TagsGenerator<'a, P>,
}

impl<'a, P: AsRef<Path> + Hash> CtagsSearcher<'a, P> {
    pub fn new(tags_generator: TagsGenerator<'a, P>) -> Self {
        let tags_path = tags_generator.tags_path();
        Self {
            tags_path,
            tags_generator,
        }
    }

    /// Returns `true` if the tags file already exists.
    pub fn tags_exists(&self) -> bool {
        self.tags_path.exists()
    }

    /// Generate the `tags` file.
    pub fn generate_tags(&self) -> std::io::Result<()> {
        self.tags_generator.generate_tags()
    }

    pub fn search_usages(
        &self,
        keyword: &str,
        usage_matcher: &UsageMatcher,
        query_type: QueryType,
        force_generate: bool,
    ) -> Result<Vec<AddressableUsage>> {
        let ignorecase = keyword.chars().all(char::is_lowercase);

        // TODO: reorder the ctags results similar to gtags.
        let usages = self
            .search_symbols(keyword, query_type, force_generate)?
            .sorted_by_key(|s| s.line_number) // Ensure the tags are sorted as the definition goes first and then the implementations.
            .par_bridge()
            .filter_map(|symbol| {
                let (line, indices) = symbol.grep_format_ctags(keyword, ignorecase);
                usage_matcher
                    .check_jump_line((line, indices.unwrap_or_default()))
                    .map(|(line, indices)| symbol.into_addressable_usage(line, indices))
            })
            .collect::<Vec<_>>();

        Ok(usages)
    }

    fn build_exec(&self, query: &str, query_type: QueryType) -> Exec {
        // https://docs.ctags.io/en/latest/man/readtags.1.html#examples
        let cmd = Exec::cmd("readtags")
            .stderr(Redirection::None) // Ignore the line: ctags: warning...
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
                .arg(format!("(eq? (downcase $name) \"{query}\")"))
                .arg("-l"),
            QueryType::Contain => cmd
                .arg("-Q")
                .arg(format!("(substr? (downcase $name) \"{query}\")"))
                .arg("-l"),
            QueryType::Inherit => {
                todo!("Inherit")
            }
        }
    }

    pub fn search_symbols(
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
