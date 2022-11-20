//! Inspired by https://github.com/jacktasia/dumb-jump/blob/master/dumb-jump.el.
//!
//! This module requires the executable rg with `--json` and `--pcre2` is installed in the system.

use crate::find_usages::{CtagsSearcher, QueryType, RegexSearcher, Usages};
use crate::tools::ctags::{get_language, TagsGenerator};
use crate::utils::ExactOrInverseTerms;
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Search-based jump.
#[derive(Parser, Debug, Clone)]
pub struct DumbJump {
    /// Search term.
    #[clap(index = 1, long)]
    pub word: String,

    /// File extension.
    #[clap(index = 2, long)]
    pub extension: String,

    /// Definition kind.
    #[clap(long)]
    pub kind: Option<String>,

    /// Specify the working directory.
    #[clap(long, parse(from_os_str))]
    pub cmd_dir: Option<PathBuf>,

    /// Use RegexSearcher instead of CtagsSearcher
    #[clap(long)]
    pub regex: bool,
}

impl DumbJump {
    pub fn run(self) -> Result<()> {
        let Self {
            word,
            extension,
            cmd_dir,
            ..
        } = self;

        if self.regex {
            let regex_searcher = RegexSearcher {
                word,
                extension,
                dir: cmd_dir,
            };
            regex_searcher.print_usages(&Default::default())?;
        } else {
            let cwd = match cmd_dir {
                Some(cwd) => cwd,
                None => std::env::current_dir()?,
            };
            let mut tags_generator = TagsGenerator::with_dir(cwd);
            if let Some(language) = get_language(&extension) {
                tags_generator.set_languages(language.into());
            }

            let ctags_searcher = CtagsSearcher::new(tags_generator);
            let usages = ctags_searcher.search_usages(
                &word,
                &Default::default(),
                QueryType::Exact,
                false,
            )?;
            println!("usages: {:#?}", usages);
        }

        Ok(())
    }

    pub fn regex_usages(
        &self,
        classify: bool,
        exact_or_inverse_terms: &ExactOrInverseTerms,
    ) -> Result<Usages> {
        let searcher = RegexSearcher {
            word: self.word.to_string(),
            extension: self.extension.to_string(),
            dir: self.cmd_dir.clone(),
        };
        Ok(searcher
            .search_usages(classify, exact_or_inverse_terms)?
            .into())
    }
}
