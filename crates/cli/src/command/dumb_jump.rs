//! Inspired by https://github.com/jacktasia/dumb-jump/blob/master/dumb-jump.el.
//!
//! This module requires the executable rg with `--json` and `--pcre2` is installed in the system.

use anyhow::Result;
use clap::Parser;
use maple_core::find_usages::{CtagsSearcher, QueryType, RegexSearcher, UsageMatcher, Usages};
use maple_core::tools::ctags::{get_language, TagsGenerator};
use std::path::PathBuf;

/// Search-based jump.
#[derive(Parser, Debug, Clone)]
pub struct DumbJump {
    /// Search term.
    #[clap(index = 1)]
    pub word: String,

    /// File extension.
    #[clap(index = 2)]
    pub extension: String,

    /// Definition kind.
    #[clap(long)]
    pub kind: Option<String>,

    /// Specify the working directory.
    #[clap(long, value_parser)]
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
            let usages = regex_searcher.cli_usages(&Default::default())?;
            let total = usages.len();
            let (lines, indices): (Vec<_>, Vec<_>) = usages
                .into_iter()
                .map(|usage| (usage.line, usage.indices))
                .unzip();
            printer::println_json_with_length!(total, lines, indices);
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
            println!("usages: {usages:#?}");
        }

        Ok(())
    }

    pub fn regex_usages(&self, classify: bool, usage_matcher: &UsageMatcher) -> Result<Usages> {
        let searcher = RegexSearcher {
            word: self.word.to_string(),
            extension: self.extension.to_string(),
            dir: self.cmd_dir.clone(),
        };
        Ok(searcher.search_usages(classify, usage_matcher)?.into())
    }
}
